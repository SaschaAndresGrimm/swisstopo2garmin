# Routing

A routable map lets the device navigate *along* the roads and paths rather than only draw
them. It is the one substantial gap against the commercial Garmin TOPO Schweiz, and
SPEC.md §16 lists it as the highest-value follow-up to v1.

**It is off by default and labelled as unverified in the app.** Nothing about it has been
checked on a device: not the road classes, not the access rules, and not the assumption
below. Turning it on is a choice a user makes with that stated.

## What routing is, on disk

Exactly two subfiles: **NET** (road segments and their attributes) and **NOD** (the node
graph and the turns between segments). Without them the device treats every road as
scenery, which looks identical on a screenshot — so
`a_routable_build_writes_the_road_network_subfiles` asserts both are present with
`--route`, absent without it, and that the output actually grew.

Two conditions have to hold together:

1. **mkgmap is given `--route`**, on *both* passes. Nothing carries over from the first.
2. **The style sets `road_class` and `road_speed`** on the road types.

And a third that made this possible at all: **the roads already used standard Garmin line
types** (`0x01`–`0x1a`). An extended type — the `0x10101` range the trail overlays and
lifts use — cannot be a road, so a cartography built entirely on extended types would
have had to be redone. It was not, because the trail overlays are drawn `continue
with_actions` over the underlying road rule, which keeps the standard type underneath.

## The one assumption, and how it was checked

`richtungsgetrennt=Wahr` marks a carriageway carrying one direction of traffic. To turn
that into `oneway=yes`, the geometry has to be **digitised in the direction of travel** —
and if it is not, a device sends a cyclist the wrong way down a dual carriageway. That is
not a thing to assume.

It is checkable from the data alone, because motorway ramps are one-way by construction:
an `Ausfahrt` leaves the motorway and an `Einfahrt` joins it. If digitisation follows
travel, an exit's *first* vertex sits on the motorway and its last does not.
`examples/probe_direction.rs` measures that over the Mittelland between Bern and Zürich —
1,471 ramps against 55,622 motorway vertices:

| minimum end-to-end asymmetry | ramps judged | agree with travel direction |
|---:|---:|---:|
| 20 m | 715 | 93.4 % |
| 100 m | 148 | 97.3 % |
| 300 m | 14 | **100 %** |

Agreement rises as the test gets stricter, which is the signature of a clean convention
plus a crude test rather than of noisy data: the disagreements are ramps whose two ends
are both near a motorway, at interchanges where "on the motorway" is ambiguous. So
swissTLM3D digitises roads in the direction of travel, and `oneway=yes` is right.

Re-run with `cargo run --release -q -p s2g-core --example probe_direction -- 300`.

## The mapping

Every value below is from [tlm3d-schema.md](tlm3d-schema.md); none is invented. The rules
live in `style/*/lines` and are action-only, falling through to the type rules.

### Road class and speed

Garmin weights a road by `road_class` (0–4) and `road_speed` (0–7). The **types are
unchanged**: only the two attributes were added, so the cartography is exactly what it was
and this change needs no visual review.

| swissTLM3D `objektart` | type | class | speed |
|---|---|---:|---:|
| `Autobahn` | 0x01 | 4 | 7 |
| `Autostrasse` | 0x02 | 4 | 5 |
| `10m Strasse` | 0x03 | 3 | 4 |
| `Einfahrt`, `Ausfahrt` | 0x08 | 3 | 3 |
| `8m Strasse` | 0x04 | 2 | 4 |
| `6m Strasse` | 0x05 | 2 | 3 |
| `4m Strasse` | 0x06 | 1 | 3 |
| `3m Strasse` | 0x06 | 1 | 2 |
| `Verbindung`, `Zufahrt`, `Dienstzufahrt`, `Platz` | 0x07 | 0 | 1 |
| `2m Weg` | 0x0a | 0 | 1 |
| `1m Weg`, `1m/2m Wegfragment`, `Markierte Spur` | 0x16 | 0 | 0 |
| `Faehre` | 0x1a | 0 | 1 |
| `Klettersteig` | 0x10104 | — | — |

`Klettersteig` is deliberately absent: a via ferrata is climbing, and it keeps its
extended type, which carries no road class and therefore no route.

**`verkehrsbedeutung` is available and deliberately unused.** It is the functional
classification — `Hochleistungsstrasse`, `Durchgangsstrasse`, `Verbindungsstrasse` — and
a better basis for routing weights than physical width. Using it would mean changing which
*type* a road is drawn as, and a cartography change needs the hardware review routing
itself has not had. Worth revisiting once routing is verified.

### Access and restrictions

| Attribute and value | Effect |
|---|---|
| `verkehrsbeschraenkung=Allgemeine Verkehrsbeschraenkung` (127,868) | `motor_vehicle=private` → reachable as a destination, not usable as a shortcut |
| `verkehrsbeschraenkung=Allgemeines Fahrverbot` | `motor_vehicle=no` |
| `verkehrsbeschraenkung=Zeitlich geregelt` | `motor_vehicle=private` |
| `verkehrsbeschraenkung=Gesperrt` | `access=no` |
| `Militaerstrasse`, `Panzerpiste` | `access=no`, `foot=yes` |
| `Teststrecke`, `Rennstrecke` | `access=no` |
| `Gesicherte Kletterpartie` | `access=no` |
| `befahrbarkeit=Falsch` | `motor_vehicle=no` |
| `kunstbaute` = a `Bruecke*` | `bridge=yes` |
| `kunstbaute` = `Tunnel`, `Galerie`, `Unterfuehrung*` | `tunnel=yes` |
| `kunstbaute` containing `Treppe` | walkable only |
| `kunstbaute=Furt` | walkable only |
| `kunstbaute=Steg` | `bridge=yes`, no motor vehicles |
| `belagsart=Natur` | `mkgmap:unpaved=1` |
| `stufe` | `layer`, so a bridge does not connect to what passes beneath it |
| `objektart=Faehre` | `mkgmap:ferry=1` |

The plain OSM tags above are then translated into the travel modes mkgmap reads —
`mkgmap:car`, `mkgmap:foot`, `mkgmap:bicycle`, `mkgmap:throughroute` and the rest. mkgmap
does that itself only in its *own* default style, so a custom style has to; the tag names
came from `styles/default/inc/access` inside the jar rather than from memory.

## What it costs

**33.7 bytes per transport feature**, measured from two builds of the same recipe with and
without `--route`:

| area | transport features | plain | routable | added | B/feature |
|---|---:|---:|---:|---:|---:|
| Grindelwald 6 km, gentle relief | 4,018 | 1,105,408 | 1,243,648 | 138,240 | 34.4 |
| Bellinzona 7 km, detailed relief | 14,238 | 2,089,984 | 2,560,000 | 470,016 | 33.0 |

Two areas 3.5× apart in road density agreeing to within 4 % is what makes it a constant
rather than a guess. It is charged **per feature, not per square kilometre**, because NET
and NOD describe a graph: an empty alpine square kilometre adds no roads and a town adds a
great many. That is exactly the weakness `SLOPE_BYTES_PER_KM2` is stuck with, and routing
does not have to repeat it — the road count is known before the build starts.

Roughly 12 % of a typical map, and the size estimate accounts for it.

## Not done

- **Turn restrictions.** swissTLM3D's road-node layer has `Durchfahrtssperre` (23,796
  barriers) and customs posts, which mkgmap could take as barrier nodes. Not wired up.
- **`verkehrsbedeutung` as the class basis**, as above.
- **Separate bicycle and foot profiles.** Every mode shares one `road_speed`, so a device
  asked for a cycling route weights a 10 m main road the way a car would.
- **Anything on hardware.** See [device-verification.md](device-verification.md) §C.7
  before trusting a route.
