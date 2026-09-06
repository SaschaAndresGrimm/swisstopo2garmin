//! OSM PBF writer (SPEC.md FR-P3..FR-P5).
//!
//! mkgmap reads `.osm`, `.o5m` and `.osm.pbf`; PBF is by far the smallest and fastest.
//! The format is protobuf, encoded here by hand: it needs three tiny message shapes,
//! which is not worth a code-generation dependency and its build step.
//!
//! Two hard constraints from Milestone 0:
//!
//! * **Element ids must ascend.** `splitter` rejects unsorted input outright
//!   ("Node ids are not sorted"). Ids are therefore allocated sequentially in
//!   emission order, and [`PbfWriter`] asserts monotonicity rather than trusting it.
//! * **Nodes must precede the ways that reference them**, so ways are buffered and
//!   flushed after the node blocks.
//!
//! Memory: the coordinate deduplication table dominates. A canton-sized extract holds
//! roughly 10 M shared nodes (~250 MB), which is within NFR-2's 4 GB budget. A
//! national single-pass extract would not be; that needs tiling, which is why
//! whole-Switzerland builds are partitioned (FR-36).

use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::{Error, Result};

/// PBF stores coordinates as `(offset + granularity * value)` nanodegrees. With the
/// default granularity of 100, a stored unit is 1e-7 degrees — about 1 cm, well below
/// Garmin's ~2.4 m grid.
const GRANULARITY: i32 = 100;
const COORD_SCALE: f64 = 1e7;

/// Elements per block. The format recommends a limit around 8000 so that a reader can
/// decode blocks independently without unbounded memory.
const BLOCK_LIMIT: usize = 8000;

pub type Tags = Vec<(String, String)>;

// ---------------------------------------------------------------------------
// protobuf primitives
// ---------------------------------------------------------------------------

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn put_key(out: &mut Vec<u8>, field: u32, wire: u32) {
    put_varint(out, ((field << 3) | wire) as u64);
}

fn put_uint(out: &mut Vec<u8>, field: u32, v: u64) {
    put_key(out, field, 0);
    put_varint(out, v);
}

fn put_sint(out: &mut Vec<u8>, field: u32, v: i64) {
    put_key(out, field, 0);
    put_varint(out, zigzag(v));
}

fn put_bytes(out: &mut Vec<u8>, field: u32, data: &[u8]) {
    put_key(out, field, 2);
    put_varint(out, data.len() as u64);
    out.extend_from_slice(data);
}

/// Packed repeated field of plain varints.
fn put_packed_uint(out: &mut Vec<u8>, field: u32, values: impl Iterator<Item = u64>) {
    let mut buf = Vec::new();
    for v in values {
        put_varint(&mut buf, v);
    }
    put_bytes(out, field, &buf);
}

/// Packed repeated field of zigzag-encoded varints.
fn put_packed_sint(out: &mut Vec<u8>, field: u32, values: impl Iterator<Item = i64>) {
    let mut buf = Vec::new();
    for v in values {
        put_varint(&mut buf, zigzag(v));
    }
    put_bytes(out, field, &buf);
}

// ---------------------------------------------------------------------------
// string table, built per block
// ---------------------------------------------------------------------------

#[derive(Default)]
struct StringTable {
    /// Index 0 is reserved by the format and must stay empty.
    strings: Vec<String>,
    index: HashMap<String, u32>,
}

impl StringTable {
    fn new() -> Self {
        Self {
            strings: vec![String::new()],
            index: HashMap::new(),
        }
    }

    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.index.get(s) {
            return i;
        }
        let i = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.index.insert(s.to_string(), i);
        i
    }

    fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        for s in &self.strings {
            put_bytes(&mut buf, 1, s.as_bytes());
        }
        let mut out = Vec::new();
        put_bytes(&mut out, 1, &buf);
        out
    }
}

// ---------------------------------------------------------------------------
// writer
// ---------------------------------------------------------------------------

struct PendingNode {
    id: i64,
    lon: i64,
    lat: i64,
    tags: Tags,
}

struct PendingWay {
    id: i64,
    refs: Vec<i64>,
    tags: Tags,
}

pub struct PbfWriter<W: Write> {
    out: W,
    nodes: Vec<PendingNode>,
    ways: Vec<PendingWay>,
    /// Shared-node lookup, keyed on the quantised coordinate (FR-P3).
    coords: HashMap<(i32, i32), i64>,
    next_id: i64,
    last_node_id: i64,
    node_count: u64,
    way_count: u64,
}

impl PbfWriter<BufWriter<std::fs::File>> {
    pub fn create(path: impl AsRef<Path>, bbox_wgs84: [f64; 4]) -> Result<Self> {
        let path = path.as_ref();
        let file = std::fs::File::create(path).map_err(|e| Error::io(path, e))?;
        PbfWriter::new(BufWriter::new(file), bbox_wgs84)
    }
}

impl<W: Write> PbfWriter<W> {
    pub fn new(mut out: W, bbox_wgs84: [f64; 4]) -> Result<Self> {
        write_header(&mut out, bbox_wgs84)?;
        Ok(Self {
            out,
            nodes: Vec::new(),
            ways: Vec::new(),
            coords: HashMap::new(),
            next_id: 1,
            last_node_id: 0,
            node_count: 0,
            way_count: 0,
        })
    }

    /// Interned node id for a WGS84 coordinate. Repeated coordinates share one node,
    /// which is required for clean rendering and is the groundwork for v2 routing.
    pub fn node_at(&mut self, lon: f64, lat: f64) -> i64 {
        let key = (
            (lon * COORD_SCALE).round() as i32,
            (lat * COORD_SCALE).round() as i32,
        );
        if let Some(&id) = self.coords.get(&key) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.coords.insert(key, id);
        self.nodes.push(PendingNode {
            id,
            lon: key.0 as i64,
            lat: key.1 as i64,
            tags: Vec::new(),
        });
        id
    }

    /// A standalone tagged node (a POI). Deliberately not interned: a POI must never
    /// merge with a way vertex, or the vertex inherits the POI's tags.
    pub fn poi(&mut self, lon: f64, lat: f64, tags: Tags) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(PendingNode {
            id,
            lon: (lon * COORD_SCALE).round() as i64,
            lat: (lat * COORD_SCALE).round() as i64,
            tags,
        });
        id
    }

    pub fn way(&mut self, refs: Vec<i64>, tags: Tags) -> Option<i64> {
        if refs.len() < 2 {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.ways.push(PendingWay { id, refs, tags });
        Some(id)
    }

    pub fn node_count(&self) -> u64 {
        self.node_count + self.nodes.len() as u64
    }
    pub fn way_count(&self) -> u64 {
        self.way_count + self.ways.len() as u64
    }

    /// Flush all buffered elements: node blocks first, then way blocks.
    pub fn finish(mut self) -> Result<(u64, u64)> {
        // Ids are handed out sequentially, so this sort is a no-op in practice. It is
        // kept because splitter rejects unsorted input, and a silent ordering bug
        // upstream would otherwise surface as a confusing failure much later.
        self.nodes.sort_by_key(|n| n.id);
        for chunk in self.nodes.chunks(BLOCK_LIMIT) {
            for n in chunk {
                if n.id <= self.last_node_id {
                    return Err(Error::Inflate(format!(
                        "node ids must ascend: {} after {}",
                        n.id, self.last_node_id
                    )));
                }
                self.last_node_id = n.id;
            }
            let block = encode_node_block(chunk);
            write_blob(&mut self.out, "OSMData", &block)?;
            self.node_count += chunk.len() as u64;
        }
        self.nodes = Vec::new();

        self.ways.sort_by_key(|w| w.id);
        for chunk in self.ways.chunks(BLOCK_LIMIT) {
            let block = encode_way_block(chunk);
            write_blob(&mut self.out, "OSMData", &block)?;
            self.way_count += chunk.len() as u64;
        }
        self.ways = Vec::new();

        self.out
            .flush()
            .map_err(|e| Error::io(Path::new("<pbf>"), e))?;
        Ok((self.node_count, self.way_count))
    }
}

fn write_header<W: Write>(out: &mut W, bbox: [f64; 4]) -> Result<()> {
    let mut hb = Vec::new();
    // HeaderBBox in nanodegrees: left, right, top, bottom.
    let mut bb = Vec::new();
    put_sint(&mut bb, 1, (bbox[0] * 1e9) as i64);
    put_sint(&mut bb, 2, (bbox[2] * 1e9) as i64);
    put_sint(&mut bb, 3, (bbox[3] * 1e9) as i64);
    put_sint(&mut bb, 4, (bbox[1] * 1e9) as i64);
    put_bytes(&mut hb, 1, &bb);

    // Declaring only the features we actually rely on keeps older readers working.
    put_bytes(&mut hb, 4, b"OsmSchema-V0.6");
    put_bytes(&mut hb, 4, b"DenseNodes");
    put_bytes(
        &mut hb,
        16,
        concat!("swisstopo2garmin ", env!("CARGO_PKG_VERSION")).as_bytes(),
    );
    put_bytes(&mut hb, 17, b"swissTLM3D (c) swisstopo");

    write_blob(out, "OSMHeader", &hb)
}

fn encode_node_block(nodes: &[PendingNode]) -> Vec<u8> {
    let mut st = StringTable::new();

    // DenseNodes: ids and coordinates are delta encoded, and keys_vals is a flat
    // stream of interned key/value pairs terminated by 0 per node.
    let mut keys_vals: Vec<u64> = Vec::new();
    let mut any_tags = false;
    for n in nodes {
        for (k, v) in &n.tags {
            keys_vals.push(st.intern(k) as u64);
            keys_vals.push(st.intern(v) as u64);
            any_tags = true;
        }
        keys_vals.push(0);
    }

    let mut dense = Vec::new();
    let mut prev = 0i64;
    put_packed_sint(
        &mut dense,
        1,
        nodes.iter().map(|n| {
            let d = n.id - prev;
            prev = n.id;
            d
        }),
    );
    let mut prev_lat = 0i64;
    put_packed_sint(
        &mut dense,
        8,
        nodes.iter().map(|n| {
            let d = n.lat - prev_lat;
            prev_lat = n.lat;
            d
        }),
    );
    let mut prev_lon = 0i64;
    put_packed_sint(
        &mut dense,
        9,
        nodes.iter().map(|n| {
            let d = n.lon - prev_lon;
            prev_lon = n.lon;
            d
        }),
    );
    if any_tags {
        put_packed_uint(&mut dense, 10, keys_vals.into_iter());
    }

    let mut group = Vec::new();
    put_bytes(&mut group, 2, &dense);

    let mut block = st.encode();
    put_bytes(&mut block, 2, &group);
    put_uint(&mut block, 17, GRANULARITY as u64);
    block
}

fn encode_way_block(ways: &[PendingWay]) -> Vec<u8> {
    let mut st = StringTable::new();
    let mut group = Vec::new();

    for w in ways {
        let keys: Vec<u64> = w.tags.iter().map(|(k, _)| st.intern(k) as u64).collect();
        let vals: Vec<u64> = w.tags.iter().map(|(_, v)| st.intern(v) as u64).collect();

        let mut way = Vec::new();
        put_uint(&mut way, 1, w.id as u64);
        if !keys.is_empty() {
            put_packed_uint(&mut way, 2, keys.into_iter());
            put_packed_uint(&mut way, 3, vals.into_iter());
        }
        let mut prev = 0i64;
        put_packed_sint(
            &mut way,
            8,
            w.refs.iter().map(|&r| {
                let d = r - prev;
                prev = r;
                d
            }),
        );
        put_bytes(&mut group, 3, &way);
    }

    let mut block = st.encode();
    put_bytes(&mut block, 2, &group);
    put_uint(&mut block, 17, GRANULARITY as u64);
    block
}

/// Write one `BlobHeader` + `Blob` pair. The header length is a big-endian int32,
/// which is the only part of the container that is not protobuf.
fn write_blob<W: Write>(out: &mut W, kind: &str, payload: &[u8]) -> Result<()> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;

    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(payload)
        .map_err(|e| Error::io(Path::new("<pbf>"), e))?;
    let compressed = enc.finish().map_err(|e| Error::io(Path::new("<pbf>"), e))?;

    let mut blob = Vec::new();
    put_uint(&mut blob, 2, payload.len() as u64); // raw_size
    put_bytes(&mut blob, 3, &compressed); // zlib_data

    let mut header = Vec::new();
    put_bytes(&mut header, 1, kind.as_bytes());
    put_uint(&mut header, 3, blob.len() as u64);

    out.write_all(&(header.len() as u32).to_be_bytes())
        .map_err(|e| Error::io(Path::new("<pbf>"), e))?;
    out.write_all(&header)
        .map_err(|e| Error::io(Path::new("<pbf>"), e))?;
    out.write_all(&blob)
        .map_err(|e| Error::io(Path::new("<pbf>"), e))?;
    Ok(())
}
