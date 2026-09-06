# ASTRA route networks — discovered schema

> **Generated, do not edit by hand.** Produced by `spikes/s0/routes_schema.py`
> from the real shapefiles.

- **Generated:** 2026-09-06 15:19 UTC
- These datasets publish **shapefile and File Geodatabase only**; there is no
  GeoPackage, which is why `crates/s2g-core/src/shapefile.rs` exists.
- Columns with more than 25 distinct values are not enumerated.

## `mountainbikeland` / `Etappe.shp`

**430** records · shape type 13 (PolyLineZ)

| Field | Type | Len | Distinct | Values |
|---|---|---:|---:|---|
| `OBJECTID` | N | 11 | >25 | _not enumerated_ |
| `Abwicklung` | C | 254 | 3 | `Hinweg` (309) · `Identischer Hin- und RÃ¼ckweg` (94) · `RÃ¼ckweg` (27) |
| `Change_Dt` | C | 21 | >25 | _not enumerated_ |
| `HoeheAbE` | N | 11 | >25 | _not enumerated_ |
| `HoeheAufE` | N | 11 | >25 | _not enumerated_ |
| `HoeheMaxE` | N | 11 | >25 | _not enumerated_ |
| `HoeheMinE` | N | 11 | >25 | _not enumerated_ |
| `KonditionE` | C | 254 | 3 | `leicht` (154) · `schwer` (150) · `mittel` (126) |
| `DistanzE` | F | 31 | >25 | _not enumerated_ |
| `LVEtappe_I` | C | 38 | >25 | _not enumerated_ |
| `LVRoute_ID` | C | 38 | >25 | _not enumerated_ |
| `NameS` | C | 254 | >25 | _not enumerated_ |
| `NameZ` | C | 254 | >25 | _not enumerated_ |
| `NrEtappe` | F | 31 | 16 | `1.000000000000000` (316) · `2.000000000000000` (26) · `3.000000000000000` (24) · `4.000000000000000` (15) · `5.000000000000000` (9) · `6.000000000000000` (6) · `11.000000000000000` (6) · `7.000000000000000` (5) · `8.000000000000000` (5) · `9.000000000000000` (4) · `10.000000000000000` (4) · `13.000000000000000` (3) |
| `TechnikE` | C | 254 | 5 | `mittel` (209) · `schwer` (113) · `leicht` (105) · _empty_ (2) · `Unbekannt` (1) |
| `NrR_ID` | C | 32 | >25 | _not enumerated_ |
| `NrR` | C | 64 | >25 | _not enumerated_ |
| `ZeitStZiE` | N | 11 | 1 | `0` (430) |
| `ZeitZiStE` | N | 11 | 1 | `0` (430) |
| `SHAPE_Leng` | F | 31 | >25 | _not enumerated_ |

## `mountainbikeland` / `MTBWeg.shp`

**52,316** records · shape type 13 (PolyLineZ)

| Field | Type | Len | Distinct | Values |
|---|---|---:|---:|---|
| `OBJECTID` | N | 11 | >25 | _not enumerated_ |
| `BelagTLM` | C | 254 | 4 | `hart` (35,514) · `Natur` (16,658) · `Unbekannt` (101) · _empty_ (43) |
| `Bverb` | C | 254 | 1 | _empty_ (52,316) |
| `BverbE` | C | 254 | 1 | _empty_ (52,316) |
| `BverbQ` | C | 254 | 1 | _empty_ (52,316) |
| `Change_Dt` | C | 21 | >25 | _not enumerated_ |
| `FuehrArt` | C | 254 | 1 | _empty_ (52,316) |
| `LVWeg_ID` | C | 38 | >25 | _not enumerated_ |
| `ReStWeg` | C | 254 | 4 | `realisiert` (52,086) · _empty_ (197) · `geplant` (26) · `aufzuheben` (7) |
| `TLM_ID` | C | 38 | >25 | _not enumerated_ |
| `VerkehrM` | C | 254 | 9 | _empty_ (52,273) · `Luftseilbahn` (14) · `Bus` (9) · `Sessellift` (9) · `Gondelbahn` (6) · `Eisenbahn` (2) · `Zahnrad- und Standseilbahn` (1) · `Anderes Verkehrsmittel` (1) · `Schiff` (1) |
| `Zustand` | C | 254 | 3 | _empty_ (51,233) · `In Ordnung` (1,082) · `Unbekannt` (1) |
| `GeigenE` | C | 254 | 6 | _empty_ (51,905) · `1` (295) · `nur FWR` (47) · `3` (46) · `4` (14) · `2` (9) |
| `IsGEigen` | C | 254 | 4 | _empty_ (35,291) · `Unbekannt` (16,880) · `Ja` (144) · `Nein` (1) |
| `IsSTrail` | N | 6 | 3 | `0` (47,436) · `1` (4,802) · _empty_ (78) |
| `Netzhier` | C | 254 | 1 | _empty_ (52,316) |
| `InfraMtb` | C | 254 | 4 | `Keine` (48,368) · _empty_ (3,748) · `MTB-spezifische Infrastruktur` (155) · `Bikepark-Infrastruktur` (45) |
| `Technik` | C | 254 | 5 | _empty_ (51,399) · `mittel` (547) · `leicht` (220) · `sehr leicht` (82) · `schwer` (68) |
| `SHAPE_Leng` | F | 31 | >25 | _not enumerated_ |

## `mountainbikeland` / `Route.shp`

**336** records · shape type 13 (PolyLineZ)

| Field | Type | Len | Distinct | Values |
|---|---|---:|---:|---|
| `OBJECTID` | N | 11 | >25 | _not enumerated_ |
| `Abwicklung` | C | 254 | 3 | `Hinweg` (266) · `Identischer Hin- und RÃ¼ckweg` (56) · `RÃ¼ckweg` (14) |
| `AOrt` | C | 254 | >25 | _not enumerated_ |
| `AuspraegR` | C | 254 | 1 | `Soll` (336) |
| `BeschreibR` | C | 254 | >25 | _not enumerated_ |
| `Change_Dt` | C | 21 | >25 | _not enumerated_ |
| `GueltigJ` | N | 11 | 14 | _empty_ (202) · `2020` (18) · `2017` (15) · `2018` (14) · `2023` (14) · `2021` (13) · `2014` (12) · `2016` (10) · `2015` (9) · `2025` (8) · `2019` (8) · `2022` (5) |
| `HoeheAbR` | N | 11 | >25 | _not enumerated_ |
| `HoeheAufR` | N | 11 | >25 | _not enumerated_ |
| `HoeheMaxR` | N | 11 | >25 | _not enumerated_ |
| `HoeheMinR` | N | 11 | >25 | _not enumerated_ |
| `KonditionR` | C | 254 | 3 | `leicht` (143) · `mittel` (105) · `schwer` (88) |
| `LaengeR` | F | 31 | >25 | _not enumerated_ |
| `LVRoute_ID` | C | 38 | >25 | _not enumerated_ |
| `ReStR` | C | 254 | 1 | `realisiert` (336) |
| `Richtung` | C | 254 | 2 | `nur in Richtung Start - Ziel` (252) · `beide` (84) |
| `Routenart` | C | 254 | 2 | `Touristische Route` (320) · `Zubringerroute` (16) |
| `TechnikR` | C | 254 | 4 | `mittel` (157) · `leicht` (86) · `schwer` (77) · _empty_ (16) |
| `NameR` | C | 254 | >25 | _not enumerated_ |
| `Typ_TR` | C | 254 | 4 | `Lokal` (288) · `Regional` (26) · _empty_ (16) · `National` (6) |
| `ZeitStZiR` | N | 11 | 1 | `0` (336) |
| `ZeitZiStR` | N | 11 | 1 | `0` (336) |
| `ZOrt` | C | 254 | >25 | _not enumerated_ |
| `NichtPubFh` | N | 6 | 1 | `0` (336) |
| `UnsEtpZiel` | N | 6 | 1 | `0` (336) |
| `NrR` | C | 64 | >25 | _not enumerated_ |
| `SHAPE_Leng` | F | 31 | >25 | _not enumerated_ |

## `veloland` / `Etappe.shp`

**424** records · shape type 13 (PolyLineZ)

| Field | Type | Len | Distinct | Values |
|---|---|---:|---:|---|
| `OBJECTID` | N | 11 | >25 | _not enumerated_ |
| `Abwicklung` | C | 254 | 3 | `Identischer Hin- und RÃ¼ckweg` (167) · `Hinweg` (129) · `RÃ¼ckweg` (128) |
| `Change_Dt` | C | 21 | >25 | _not enumerated_ |
| `HoeheAbE` | N | 11 | >25 | _not enumerated_ |
| `HoeheAufE` | N | 11 | >25 | _not enumerated_ |
| `HoeheMaxE` | N | 11 | >25 | _not enumerated_ |
| `HoeheMinE` | N | 11 | >25 | _not enumerated_ |
| `KonditionE` | C | 254 | 3 | `mittel` (226) · `schwer` (107) · `leicht` (91) |
| `DistanzE` | F | 31 | >25 | _not enumerated_ |
| `LVEtappe_I` | C | 38 | >25 | _not enumerated_ |
| `LVRoute_ID` | C | 38 | >25 | _not enumerated_ |
| `NameS` | C | 254 | >25 | _not enumerated_ |
| `NameZ` | C | 254 | >25 | _not enumerated_ |
| `NrEtappe` | F | 31 | 13 | `1.000000000000000` (174) · `2.000000000000000` (93) · `3.000000000000000` (45) · `4.000000000000000` (33) · `5.000000000000000` (20) · `6.000000000000000` (19) · `7.000000000000000` (16) · `8.000000000000000` (12) · `9.000000000000000` (5) · `10.000000000000000` (3) · `13.000000000000000` (2) · `11.000000000000000` (1) |
| `TechnikE` | C | 254 | 5 | `mittel` (216) · `leicht` (158) · _empty_ (35) · `schwer` (13) · `Unbekannt` (2) |
| `NrR_ID` | C | 32 | >25 | _not enumerated_ |
| `NrR` | C | 64 | >25 | _not enumerated_ |
| `ZeitStZiE` | N | 11 | >25 | _not enumerated_ |
| `ZeitZiStE` | N | 11 | >25 | _not enumerated_ |
| `SHAPE_Leng` | F | 31 | >25 | _not enumerated_ |

## `veloland` / `Route.shp`

**309** records · shape type 13 (PolyLineZ)

| Field | Type | Len | Distinct | Values |
|---|---|---:|---:|---|
| `OBJECTID` | N | 11 | >25 | _not enumerated_ |
| `Abwicklung` | C | 254 | 3 | `Identischer Hin- und RÃ¼ckweg` (154) · `Hinweg` (80) · `RÃ¼ckweg` (75) |
| `AOrt` | C | 254 | >25 | _not enumerated_ |
| `AuspraegR` | C | 254 | 1 | `Soll` (309) |
| `BeschreibR` | C | 254 | >25 | _not enumerated_ |
| `Change_Dt` | C | 21 | >25 | _not enumerated_ |
| `GueltigJ` | N | 11 | 12 | _empty_ (264) · `2017` (9) · `2021` (6) · `2014` (5) · `2019` (5) · `2023` (5) · `2016` (4) · `2025` (4) · `2015` (2) · `2018` (2) · `2024` (2) · `2020` (1) |
| `HoeheAbR` | N | 11 | >25 | _not enumerated_ |
| `HoeheAufR` | N | 11 | >25 | _not enumerated_ |
| `HoeheMaxR` | N | 11 | >25 | _not enumerated_ |
| `HoeheMinR` | N | 11 | >25 | _not enumerated_ |
| `KonditionR` | C | 254 | 3 | `leicht` (150) · `mittel` (121) · `schwer` (38) |
| `LaengeR` | F | 31 | >25 | _not enumerated_ |
| `LVRoute_ID` | C | 38 | >25 | _not enumerated_ |
| `ReStR` | C | 254 | 1 | `realisiert` (309) |
| `Richtung` | C | 254 | 2 | `beide` (304) · `nur in Richtung Start - Ziel` (5) |
| `Routenart` | C | 254 | 4 | `Touristische Route` (195) · `Zubringerroute` (112) · `Variante` (1) · `Nebenroute` (1) |
| `TechnikR` | C | 254 | 4 | `mittel` (145) · `leicht` (87) · _empty_ (70) · `schwer` (7) |
| `NameR` | C | 254 | >25 | _not enumerated_ |
| `Typ_TR` | C | 254 | 5 | _empty_ (112) · `Regional` (97) · `Lokal` (79) · `National` (19) · `International` (2) |
| `ZeitStZiR` | N | 11 | 1 | `0` (309) |
| `ZeitZiStR` | N | 11 | 1 | `0` (309) |
| `ZOrt` | C | 254 | >25 | _not enumerated_ |
| `NichtPubFh` | N | 6 | 1 | `0` (309) |
| `UnsEtpZiel` | N | 6 | 1 | `0` (309) |
| `NrR` | C | 64 | >25 | _not enumerated_ |
| `SHAPE_Leng` | F | 31 | >25 | _not enumerated_ |

## `veloland` / `VeloWeg.shp`

**88,939** records · shape type 13 (PolyLineZ)

| Field | Type | Len | Distinct | Values |
|---|---|---:|---:|---|
| `OBJECTID` | N | 11 | >25 | _not enumerated_ |
| `BelagTLM` | C | 254 | 4 | `hart` (83,750) · `Natur` (4,744) · `Unbekannt` (442) · _empty_ (3) |
| `Bverb` | C | 254 | 1 | _empty_ (88,939) |
| `BverbE` | C | 254 | 1 | _empty_ (88,939) |
| `BverbQ` | C | 254 | 1 | _empty_ (88,939) |
| `Change_Dt` | C | 21 | >25 | _not enumerated_ |
| `FuehrArt` | C | 254 | 1 | _empty_ (88,939) |
| `LVWeg_ID` | C | 38 | >25 | _not enumerated_ |
| `ReStWeg` | C | 254 | 3 | `realisiert` (88,837) · `geplant` (85) · _empty_ (17) |
| `TLM_ID` | C | 38 | >25 | _not enumerated_ |
| `VerkehrM` | C | 254 | 4 | _empty_ (88,936) · `Anderes Verkehrsmittel` (1) · `Eisenbahn` (1) · `Schiff` (1) |
| `Zustand` | C | 254 | 3 | _empty_ (87,154) · `In Ordnung` (1,710) · `Unbekannt` (75) |
| `GeigenE` | C | 254 | 8 | _empty_ (88,760) · `2e` (61) · `2` (60) · `1` (29) · `nur FWR` (12) · `Waldstrasse / im RP bezeichnet` (9) · `FWR` (7) · `FWR-Plan` (1) |
| `IsGEigen` | C | 254 | 4 | _empty_ (71,967) · `Unbekannt` (16,723) · `Ja` (240) · `Nein` (9) |
| `Netzhier` | C | 254 | 1 | _empty_ (88,939) |
| `ObjektArt` | C | 254 | 12 | `4m Strasse` (37,824) · `3m Strasse` (20,660) · `6m Strasse` (15,126) · `2m Weg` (9,867) · `8m Strasse` (2,746) · `10m Strasse` (1,553) · `1m Weg` (610) · `Verbindung` (455) · `Platz` (79) · _empty_ (17) · `Ausfahrt` (1) · `Einfahrt` (1) |
| `SHAPE_Leng` | F | 31 | >25 | _not enumerated_ |

## `wanderland` / `Etappe.shp`

**893** records · shape type 13 (PolyLineZ)

| Field | Type | Len | Distinct | Values |
|---|---|---:|---:|---|
| `OBJECTID` | N | 11 | >25 | _not enumerated_ |
| `Abwicklung` | C | 254 | 3 | `Identischer Hin- und RÃ¼ckweg` (881) · `Hinweg` (6) · `RÃ¼ckweg` (6) |
| `Change_Dt` | C | 21 | >25 | _not enumerated_ |
| `HoeheAbE` | N | 11 | >25 | _not enumerated_ |
| `HoeheAufE` | N | 11 | >25 | _not enumerated_ |
| `HoeheMaxE` | N | 11 | >25 | _not enumerated_ |
| `HoeheMinE` | N | 11 | >25 | _not enumerated_ |
| `KonditionE` | C | 254 | 3 | `schwer` (459) · `mittel` (385) · `leicht` (49) |
| `DistanzE` | F | 31 | >25 | _not enumerated_ |
| `LVEtappe_I` | C | 38 | >25 | _not enumerated_ |
| `LVRoute_ID` | C | 38 | >25 | _not enumerated_ |
| `NameS` | C | 254 | >25 | _not enumerated_ |
| `NameZ` | C | 254 | >25 | _not enumerated_ |
| `NrEtappe` | F | 31 | >25 | _not enumerated_ |
| `TechnikE` | C | 254 | 4 | `mittel` (469) · `leicht` (420) · _empty_ (3) · `schwer` (1) |
| `NrR_ID` | C | 32 | >25 | _not enumerated_ |
| `NrR` | C | 64 | >25 | _not enumerated_ |
| `ZeitStZiE` | N | 11 | >25 | _not enumerated_ |
| `ZeitZiStE` | N | 11 | >25 | _not enumerated_ |
| `SHAPE_Leng` | F | 31 | >25 | _not enumerated_ |

## `wanderland` / `Route.shp`

**359** records · shape type 13 (PolyLineZ)

| Field | Type | Len | Distinct | Values |
|---|---|---:|---:|---|
| `OBJECTID` | N | 11 | >25 | _not enumerated_ |
| `Abwicklung` | C | 254 | 1 | `Identischer Hin- und RÃ¼ckweg` (359) |
| `AOrt` | C | 254 | >25 | _not enumerated_ |
| `AuspraegR` | C | 254 | 1 | `Soll` (359) |
| `BeschreibR` | C | 254 | >25 | _not enumerated_ |
| `Change_Dt` | C | 21 | >25 | _not enumerated_ |
| `GueltigJ` | N | 11 | 14 | _empty_ (148) · `2022` (53) · `2021` (50) · `2020` (28) · `2019` (13) · `2025` (10) · `2016` (10) · `2023` (9) · `2017` (9) · `2015` (8) · `2014` (7) · `2018` (7) |
| `HoeheAbR` | N | 11 | >25 | _not enumerated_ |
| `HoeheAufR` | N | 11 | >25 | _not enumerated_ |
| `HoeheMaxR` | N | 11 | >25 | _not enumerated_ |
| `HoeheMinR` | N | 11 | >25 | _not enumerated_ |
| `KonditionR` | C | 254 | 3 | `mittel` (200) · `schwer` (126) · `leicht` (33) |
| `LaengeR` | F | 31 | >25 | _not enumerated_ |
| `LVRoute_ID` | C | 38 | >25 | _not enumerated_ |
| `ReStR` | C | 254 | 1 | `realisiert` (359) |
| `Richtung` | C | 254 | 1 | `beide` (359) |
| `Routenart` | C | 254 | 4 | `Touristische Route` (351) · `Zubringerroute` (4) · `Nebenroute` (2) · `Variante` (2) |
| `TechnikR` | C | 254 | 3 | `mittel` (210) · `leicht` (145) · _empty_ (4) |
| `NameR` | C | 254 | >25 | _not enumerated_ |
| `Typ_TR` | C | 254 | 4 | `Lokal` (273) · `Regional` (71) · `National` (11) · _empty_ (4) |
| `ZeitStZiR` | N | 11 | >25 | _not enumerated_ |
| `ZeitZiStR` | N | 11 | >25 | _not enumerated_ |
| `ZOrt` | C | 254 | >25 | _not enumerated_ |
| `NichtPubFh` | N | 6 | 1 | `0` (359) |
| `UnsEtpZiel` | N | 6 | 1 | `0` (359) |
| `NrR` | C | 64 | >25 | _not enumerated_ |
| `SHAPE_Leng` | F | 31 | >25 | _not enumerated_ |

## `wanderland` / `WanderWeg.shp`

**73,969** records · shape type 13 (PolyLineZ)

| Field | Type | Len | Distinct | Values |
|---|---|---:|---:|---|
| `OBJECTID` | N | 11 | >25 | _not enumerated_ |
| `BelagTLM` | C | 254 | 4 | `hart` (41,062) · `Natur` (32,393) · `Unbekannt` (489) · _empty_ (25) |
| `Bverb` | C | 254 | 1 | _empty_ (73,969) |
| `BverbE` | C | 254 | 1 | _empty_ (73,969) |
| `BverbQ` | C | 254 | 1 | _empty_ (73,969) |
| `Change_Dt` | C | 21 | >25 | _not enumerated_ |
| `FuehrArt` | C | 254 | 1 | _empty_ (73,969) |
| `LVWeg_ID` | C | 38 | >25 | _not enumerated_ |
| `ReStWeg` | C | 254 | 4 | `realisiert` (73,762) · `aufzuheben` (110) · `geplant` (78) · _empty_ (19) |
| `TLM_ID` | C | 38 | >25 | _not enumerated_ |
| `VerkehrM` | C | 254 | 5 | _empty_ (73,957) · `Schiff` (6) · `Zahnrad- und Standseilbahn` (3) · `Luftseilbahn` (2) · `Bus` (1) |
| `Zustand` | C | 254 | 4 | _empty_ (28,892) · `In Ordnung` (27,669) · `Unbekannt` (17,340) · `Mangelhaft` (68) |
| `GeigenE` | C | 254 | 6 | _empty_ (73,948) · `Pim 2025` (8) · `Baugesuch` (7) · `commune de Croy, cf courrier du19 septembre 2019 et extrait du PV de la saÃ©nce de Muni du 44.11.2019` (4) · `PIM 2025` (1) · `Chemin actif en 2022, voir en automne si l'autre est rÃ©alisÃ© ou pas` (1) |
| `IsGEigen` | C | 254 | 4 | _empty_ (66,633) · `Unbekannt` (7,167) · `Nein` (124) · `Ja` (45) |
| `WegKat` | C | 254 | 3 | `Wanderweg` (57,835) · `Bergwanderweg` (16,119) · _empty_ (15) |
| `Netzhier` | C | 254 | 1 | _empty_ (73,969) |
| `SHAPE_Leng` | F | 31 | >25 | _not enumerated_ |
