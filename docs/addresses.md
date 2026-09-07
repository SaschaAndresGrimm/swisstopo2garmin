# Address search

Searching a street and house number on the device (SPEC.md §16 v2, "Address & POI
search"). Off by default: it needs a separate 137 MB download and adds a searchable point
per building.

## Where house numbers come from

**Not from swissTLM3D.** Its `tlm_strassen_strassenname` layer has 173,860 street *names*
with commune, canton and language — and no house numbers. The whole schema was checked;
there is no address column anywhere in it.

They come from **`ch.swisstopo.amtliches-gebaeudeadressverzeichnis`**, the official
directory of building addresses, which is binding for Swiss public authorities. It was not
obvious the dataset existed: it does not appear in the first page of the STAC catalogue,
and a search of that first page came back empty. Paging all 512 collections found it.

- **3,302,043 addresses**, Switzerland and Liechtenstein
- A semicolon-separated **CSV** in LV95 — no new parser, the same shape as swissNAMES3D
- 137 MB zipped, **468 MB** unpacked
- Licence: the link on the collection is `swisstopo.admin.ch/ogd-conditions`, the **same
  OGD terms** as every other dataset here. The STAC `license` field says `proprietary`,
  which is its placeholder rather than a statement about these terms.

The columns used: `STN_LABEL`, `ADR_NUMBER`, `ZIP_LABEL`, `ADR_STATUS`, `ADR_OFFICIAL`,
`ADR_EASTING`, `ADR_NORTHING`. Found **by name, not by position** — the register is
republished daily and a column order is nobody's promise. A reader counting positions
would read coordinates out of the wrong fields, silently.

## Reading it

`addresses::read_in_bbox` streams the CSV a line at a time and discards anything outside
the build area before touching the strings. Holding the file would be 468 MB on its own
and breach NFR-2.

On the real register: **3.3 million rows in 1.5 seconds**, 320 MB/s, constant memory,
4,447 addresses kept for Grindelwald at 6 km, zero malformed rows.

Two kinds of row are dropped, and both matter:

* `ADR_STATUS != real` — planned and retired addresses, which should not be navigable.
* `ADR_OFFICIAL != true` — 870,140 rows. A building with several addresses lists one as
  official and the rest as alternates for the same door. Indexing all of them would offer
  the user a choice between duplicates of one place.

## Getting them into the map

mkgmap's `--housenumbers` matches a node carrying `addr:housenumber` to a street within
**150 m** whose `mkgmap:street` equals the node's `addr:street`. So three things have to
line up, and none of them is visible in a screenshot:

1. Address points carry `addr:street`, `addr:housenumber`, plus `mkgmap:city` and
   `mkgmap:postal_code` — the last two because the option's own documentation asks for
   them, so a street running through two villages still resolves.
2. **Roads carry `mkgmap:street`**, set in `style/*/lines` from `tlm:strassenname`
   alongside the name. Without it every address is indexed with no street to belong to.
3. mkgmap gets `--housenumbers`, on the first pass only — the second combines finished
   tiles and the matching has already happened.

Addresses are written as POIs rather than interned vertices: an address must never merge
with a road vertex, or the road inherits a house number.

### What it looks like on disk

Grindelwald at 6 km, the same recipe with and without addresses:

| subfile | plain | with addresses |
|---|---:|---:|
| NET (streets and house numbers) | 0 | **44,140** |
| MDR (search index) | 25,246 | 27,079 |
| total | 1,105,408 | 1,155,072 |

NET appearing from nothing is the whole feature. `addresses_reach_the_search_index`
asserts it, and asserts MDR grows — because if `addr:street` and `mkgmap:street` disagree,
`--housenumbers` silently indexes nothing and the map looks fine.

## What it costs

**10.5 bytes per address**, measured:

| area | addresses | added | B/address |
|---|---:|---:|---:|
| Grindelwald 6 km | 4,888 | 49,664 | 10.2 |
| Bellinzona 7 km | 14,829 | 158,720 | 10.7 |

Three times apart in count and within 5 %.

**The estimate's weak half is the count, not the cost.** How many addresses an area holds
cannot be known before a build without scanning 468 MB, which is far too slow for an
estimate that refires as the user drags a corner. So it is predicted from the building
count, which the R-tree answers instantly — and that ratio is *not* stable: 0.75 in
Grindelwald against 0.42 in Bellinzona, because alpine barns and huts are buildings
without official addresses while a town has apartment blocks with one address each.

`ADDRESSES_PER_BUILDING` is 0.59, the middle of that. It matters less than it looks: the
whole address term is 4.4 % of the Grindelwald map, so being 30 % wrong about the count
moves the estimate by about 1.3 %. Predicting the term at all is worth much more than
predicting it precisely.

## Not done

- **Nothing on hardware.** Whether an Edge or a fēnix will actually search these addresses
  is unverified. See [device-verification.md](device-verification.md) §C.8.
- **Liechtenstein** is published as its own item and is skipped: taking both files would
  duplicate every address that exists in neither.
- **POI search beyond addresses.** Settlement names, huts and transit stops are already
  named and in the index; nobody has checked what a device makes of them.
- **`BDG_NAME`** — the register names some buildings (schools, stations). Unused.
