//! STAC parsing and multihash decoding, against recorded response shapes.

use s2g_core::stac::{Digest, Stac};
use s2g_core::testing::FakeHttp;

/// Shape recorded from data.geo.admin.ch on 2026-09-06.
fn tlm3d_items() -> serde_json::Value {
    serde_json::json!({
      "features": [
        {
          "id": "swisstlm3d_2024-03",
          "properties": {"datetime": "2024-03-06T00:00:00Z"},
          "assets": {
            "swisstlm3d_2024-03_2056_5728.gpkg.zip": {
              "href": "https://example.test/2024.gpkg.zip",
              "type": "application/x.geopackage+zip",
              "file:checksum": "1220185FFCA883E7A2D6BE4EA3D1F62EE4D1A09F07FE90E381738FDCB3A849DF28DA"
            }
          }
        },
        {
          "id": "swisstlm3d_2026-02",
          "properties": {"datetime": "2026-02-24T00:00:00Z"},
          "assets": {
            "swisstlm3d_2026-02_2056_5728.gpkg.zip": {
              "href": "https://example.test/2026.gpkg.zip",
              "type": "application/x.geopackage+zip",
              "file:checksum": "12202218054B21C23E1683FB14A80A9C33D811C7E02A891AB4AEDAF761B68750206A"
            },
            "swisstlm3d_2026-02_2056_5728.shp.zip": {
              "href": "https://example.test/2026.shp.zip",
              "type": "application/x.shapefile+zip"
            }
          }
        }
      ],
      "links": []
    })
}

#[tokio::test]
async fn resolves_the_newest_release_not_the_first() {
    let url = "http://stac.test/collections/ch.swisstopo.swisstlm3d/items?limit=100";
    let http = FakeHttp::new().with_json(url, tlm3d_items());
    let stac = Stac::with_root(&http, "http://stac.test");

    let latest = stac.latest("ch.swisstopo.swisstlm3d").await.unwrap();
    assert_eq!(latest.id, "swisstlm3d_2026-02");
}

#[tokio::test]
async fn selects_asset_by_suffix_and_reports_alternatives_when_missing() {
    let url = "http://stac.test/collections/ch.swisstopo.swisstlm3d/items?limit=100";
    let http = FakeHttp::new().with_json(url, tlm3d_items());
    let stac = Stac::with_root(&http, "http://stac.test");
    let item = stac.latest("ch.swisstopo.swisstlm3d").await.unwrap();

    let gpkg = item.asset_ending(".gpkg.zip").unwrap();
    assert_eq!(gpkg.href, "https://example.test/2026.gpkg.zip");

    let err = item.asset_ending(".gdb.zip").unwrap_err().to_string();
    assert!(
        err.contains("gpkg.zip"),
        "error should list what IS available: {err}"
    );
}

#[test]
fn decodes_the_swisstopo_multihash() {
    // 0x12 = sha2-256, 0x20 = 32 bytes
    let d = Digest::from_multihash(
        "12202218054B21C23E1683FB14A80A9C33D811C7E02A891AB4AEDAF761B68750206A",
    )
    .unwrap();
    assert_eq!(d.algo, "sha2-256");
    assert_eq!(
        d.hex,
        "2218054b21c23e1683fb14a80a9c33d811c7e02a891ab4aedaf761b68750206a"
    );
}

#[test]
fn rejects_multihashes_we_cannot_verify() {
    // 0x11 = sha1: must be refused rather than silently skipped
    assert!(Digest::from_multihash("1114aabbccddeeff00112233445566778899aabb").is_err());
    // length byte disagrees with payload
    assert!(Digest::from_multihash("1220aabb").is_err());
    assert!(Digest::from_multihash("not hex").is_err());
}

#[tokio::test]
async fn missing_assets_do_not_panic_on_malformed_features() {
    let url = "http://stac.test/collections/x/items?limit=100";
    let http = FakeHttp::new().with_json(
        url,
        serde_json::json!({"features": [{"id": "a"}, {"nonsense": true}], "links": []}),
    );
    let stac = Stac::with_root(&http, "http://stac.test");
    let items = stac.items("x", 5).await.unwrap();
    assert_eq!(items.len(), 2);
    assert!(items[0].assets.is_empty());
}

/// Every packaging swisstopo and ASTRA actually publish, taken from the real catalog.
///
/// These four shapes cost a broken Data screen: acquisition asked for `.gpkg.zip` and
/// three of the six route and winter collections do not publish one.
#[test]
fn the_primary_asset_is_chosen_by_packaging_not_by_collection() {
    use s2g_core::stac::{AssetKind, Item};

    fn item(id: &str, assets: &[&str]) -> Item {
        let map: serde_json::Map<String, serde_json::Value> = assets
            .iter()
            .map(|n| {
                (
                    (*n).to_string(),
                    serde_json::json!({ "href": format!("https://example.test/{n}") }),
                )
            })
            .collect();
        let doc = serde_json::json!({
            "features": [{ "id": id, "assets": map }]
        });
        s2g_core::stac::parse_items_for_test(&doc).remove(0)
    }

    // swissTLM3D and the SAC ski tours: a zipped GeoPackage.
    let tlm = item(
        "swisstlm3d_2026-02",
        &["swisstlm3d_2026-02_2056_5728.gpkg.zip"],
    );
    let (a, kind) = tlm.primary_asset().unwrap();
    assert_eq!(kind, AssetKind::ZippedGeoPackage);
    assert!(a.name.ends_with(".gpkg.zip"));

    // The ASTRA snowshoe and winter hiking trails: a bare GeoPackage, no archive.
    let snow = item(
        "schneeschuhwanderwege",
        &["schneeschuhwanderwege_2056.gpkg"],
    );
    let (a, kind) = snow.primary_asset().unwrap();
    assert_eq!(kind, AssetKind::PlainGeoPackage);
    assert_eq!(a.name, "schneeschuhwanderwege_2056.gpkg");
    assert!(!kind.is_archive());

    // The ASTRA route networks: shapefiles, plus a file geodatabase and a bare .zip
    // that must not be mistaken for either.
    let velo = item(
        "veloland",
        &[
            "veloland.zip",
            "veloland_2056.gdb.zip",
            "veloland_2056.shp.zip",
        ],
    );
    let (a, kind) = velo.primary_asset().unwrap();
    assert_eq!(kind, AssetKind::ZippedShapefiles);
    assert_eq!(a.name, "veloland_2056.shp.zip");

    // A GeoPackage wins when both are offered.
    let both = item("both", &["x_2056.shp.zip", "x_2056.gpkg.zip"]);
    assert_eq!(both.primary_asset().unwrap().1, AssetKind::ZippedGeoPackage);

    // Nothing usable is an error naming what was there, not a silent empty download.
    let none = item("meta-only", &["additional-files.zip", "readme.pdf"]);
    let err = none.primary_asset().unwrap_err().to_string();
    assert!(err.contains("meta-only"), "{err}");
    assert!(err.contains("readme.pdf"), "{err}");
}

/// SPEC.md §12: "STAC API unreachable — fall back to cached catalog; state that
/// release info may be stale."
///
/// Release ids are never hard-coded, so before this an unreachable API meant the Data
/// screen could say nothing at all about a dataset the user already had on disk.
#[tokio::test]
async fn an_unreachable_api_falls_back_to_the_last_answer_and_says_it_is_stale() {
    let dir = tempfile::tempdir().unwrap();
    let url = "https://example.test/collections/ch.swisstopo.swisstlm3d/items?limit=100";

    // First, a reachable API. The answer is remembered.
    let online = FakeHttp::new().with_json(url, tlm3d_items());
    let fresh = Stac::with_root(&online, "https://example.test")
        .latest_cached(
            "ch.swisstopo.swisstlm3d",
            dir.path(),
            "2026-09-07T12:00:00Z",
        )
        .await
        .unwrap();
    assert_eq!(fresh.item.id, "swisstlm3d_2026-02");
    assert!(!fresh.stale, "a live answer must not be marked stale");

    // Then the same lookup with nothing answering at all.
    let offline = FakeHttp::new();
    let remembered = Stac::with_root(&offline, "https://example.test")
        .latest_cached(
            "ch.swisstopo.swisstlm3d",
            dir.path(),
            "2026-09-14T12:00:00Z",
        )
        .await
        .unwrap();

    assert_eq!(remembered.item.id, "swisstlm3d_2026-02");
    assert!(remembered.stale, "a remembered answer must be marked stale");
    // The timestamp is the one from the call that produced it, not from now, so the UI
    // can say how old the answer is rather than only that it is old.
    assert_eq!(remembered.fetched_at, "2026-09-07T12:00:00Z");
    // The whole asset list survives, since acquiring needs the href and the checksum.
    let (asset, _) = remembered.item.primary_asset().unwrap();
    assert_eq!(asset.href, "https://example.test/2026.gpkg.zip");
    assert!(asset.checksum.is_some(), "a stale item must still verify");
}

/// With no cached answer either, the reported error must be the connection failure.
/// Inventing or guessing a release id would be worse than saying nothing.
#[tokio::test]
async fn an_unreachable_api_with_no_cached_answer_reports_the_connection_failure() {
    let dir = tempfile::tempdir().unwrap();
    let offline = FakeHttp::new();
    let err = Stac::with_root(&offline, "https://example.test")
        .latest_cached("ch.swisstopo.swisstlm3d", dir.path(), "now")
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        !msg.contains("swisstlm3d_"),
        "the error must not name a release it did not learn: {msg}"
    );
}

/// A half-written cache file must not be presented as a release.
#[tokio::test]
async fn a_corrupt_cached_answer_is_not_used() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("ch.swisstopo.swisstlm3d.json"),
        b"{\"item\": {\"id\": \"trunca",
    )
    .unwrap();
    let offline = FakeHttp::new();
    assert!(Stac::with_root(&offline, "https://example.test")
        .latest_cached("ch.swisstopo.swisstlm3d", dir.path(), "now")
        .await
        .is_err());
}

/// A later successful lookup must replace the remembered one, or the cache would pin
/// the app to whatever release it first saw.
#[tokio::test]
async fn a_new_release_replaces_the_remembered_one() {
    let dir = tempfile::tempdir().unwrap();
    let url = "https://example.test/collections/ch.swisstopo.swisstlm3d/items?limit=100";

    let first = FakeHttp::new().with_json(url, tlm3d_items());
    Stac::with_root(&first, "https://example.test")
        .latest_cached(
            "ch.swisstopo.swisstlm3d",
            dir.path(),
            "2026-09-07T12:00:00Z",
        )
        .await
        .unwrap();

    let mut newer = tlm3d_items();
    newer["features"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "swisstlm3d_2027-03",
            "properties": {"datetime": "2027-03-01T00:00:00Z"},
            "assets": {
                "swisstlm3d_2027-03_2056_5728.gpkg.zip": {
                    "href": "https://example.test/2027.gpkg.zip",
                    "type": "application/x.geopackage+zip"
                }
            }
        }));
    let second = FakeHttp::new().with_json(url, newer);
    Stac::with_root(&second, "https://example.test")
        .latest_cached(
            "ch.swisstopo.swisstlm3d",
            dir.path(),
            "2027-03-02T12:00:00Z",
        )
        .await
        .unwrap();

    let offline = FakeHttp::new();
    let remembered = Stac::with_root(&offline, "https://example.test")
        .latest_cached("ch.swisstopo.swisstlm3d", dir.path(), "later")
        .await
        .unwrap();
    assert_eq!(remembered.item.id, "swisstlm3d_2027-03");
    assert_eq!(remembered.fetched_at, "2027-03-02T12:00:00Z");
}
