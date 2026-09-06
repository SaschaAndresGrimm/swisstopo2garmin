# Winter sport routes — discovered schema

> **Generated, do not edit by hand.** Produced by `spikes/s0/winter_schema.py`
> from the real GeoPackages. Every name and value is read out of the files.

- **Generated:** 2026-09-06 14:49 UTC
- Columns with more than 40 distinct values are not enumerated.

## `schneeschuhwanderwege_2056.gpkg`

Source collection: `ch.astra.schneeschuhwanderwege`

### `Datenstand_Kantone`

**27** rows · geometry `MULTIPOLYGON` · primary key `fid` · R-tree index

| Column | Type | Distinct | Values |
|---|---|---:|---|
| `fid` | INTEGER | | |
| `geom` | MULTIPOLYGON | | |
| `uuid` | TEXT(38) | | |
| `herkunft` | TEXT(50) | 2 | `AV` (17) · `swisstopo` (10) |

## `ski_network_2056.gpkg`

Source collection: `ch.swisstopo-karto.skitouren`

### `ski_network_2056`

**19,915** rows · geometry `MULTILINESTRING` · primary key `fid` · R-tree index

| Column | Type | Distinct | Values |
|---|---|---:|---|
| `fid` | INTEGER | | |
| `geom` | MULTILINESTRING | | |
| `segm_id` | INTEGER | >40 | _not enumerated_ |

## `ski_routes_2056.gpkg`

Source collection: `ch.swisstopo-karto.skitouren`

### `ski_routes_2056`

**10,789** rows · geometry `MULTILINESTRING` · primary key `fid` · R-tree index

| Column | Type | Distinct | Values |
|---|---|---:|---|
| `fid` | INTEGER | | |
| `geom` | MULTILINESTRING | | |
| `sacid` | INTEGER | >40 | _not enumerated_ |

## `winterwanderwege_2056.gpkg`

Source collection: `ch.astra.winterwanderwege`

### `Datenstand_Kantone`

**27** rows · geometry `MULTIPOLYGON` · primary key `fid` · R-tree index

| Column | Type | Distinct | Values |
|---|---|---:|---|
| `fid` | INTEGER | | |
| `geom` | MULTIPOLYGON | | |
| `uuid` | TEXT(38) | | |
| `herkunft` | TEXT(50) | 2 | `AV` (17) · `swisstopo` (10) |
