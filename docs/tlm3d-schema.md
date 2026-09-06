# swissTLM3D — discovered schema

> **Generated, do not edit by hand.** Produced by `spikes/s0/schema_dump.py` from the
> actual GeoPackage. Every name and value below is read out of the file — nothing is
> inferred (CLAUDE.md rule 2). Regenerate after every swissTLM3D release.

- **Release:** `swisstlm3d_2026-02` (2026-02-24T00:00:00Z)
- **File:** `SWISSTLM3D_2026_LV95_LN02.gpkg` — 10.0 GB (10,777,276,416 bytes)
- **Checksum (STAC multihash):** `12202218054B21C23E1683FB14A80A9C33D811C7E02A891AB4AEDAF761B68750206A`
- **Generated:** 2026-09-06 09:00 UTC
- **Spatial layers:** 32 · **Attribute tables:** 9 · **Other:** 0
- Columns with more than 300 distinct values are reported as high-cardinality rather than enumerated.

## Spatial layers

Total features across all spatial layers: **21,730,349**

| Layer | Geometry | SRID | Features | R-tree | Identifier |
|---|---|---:|---:|:---:|---|
| `tlm_bb_einzelbaum` | POINT | 2056 | 11,494,720 | yes | tlm_bb_einzelbaum |
| `tlm_bauten_gebaeude_footprint` | POLYGON | 2056 | 4,125,163 | yes | tlm_bauten_gebaeude_footprint |
| `tlm_strassen_strasse` | LINESTRING | 2056 | 2,096,632 | yes | tlm_strassen_strasse |
| `tlm_strassen_strasseninfo` | POINT | 2056 | 1,654,515 | yes | tlm_strassen_strasseninfo |
| `tlm_bb_bodenbedeckung` | POLYGON | 2056 | 848,065 | yes | tlm_bb_bodenbedeckung |
| `tlm_gewaesser_fliessgewaesser` | LINESTRING | 2056 | 671,334 | yes | tlm_gewaesser_fliessgewaesser |
| `tlm_namen_flurname` | POINT | 2056 | 279,477 | yes | tlm_namen_flurname |
| `tlm_bauten_verbauung` | LINESTRING | 2056 | 83,109 | yes | tlm_bauten_verbauung |
| `tlm_bauten_staubaute` | POLYGON | 2056 | 73,219 | yes | tlm_bauten_staubaute |
| `tlm_oev_eisenbahn` | LINESTRING | 2056 | 56,026 | yes | tlm_oev_eisenbahn |
| `tlm_namen_siedlungsname` | POLYGON | 2056 | 54,885 | yes | tlm_namen_siedlungsname |
| `tlm_namen_siedlungsname_zentrum` | POINT | 2056 | 54,885 | yes | tlm_namen_siedlungsname_zentrum |
| `tlm_gewaesser_stehendes_gewaesser` | LINESTRING | 2056 | 43,204 | yes | tlm_gewaesser_stehendes_gewaesser |
| `tlm_areale_nutzungsareal` | POLYGON | 2056 | 36,368 | yes | tlm_areale_nutzungsareal |
| `tlm_oev_haltestelle` | POINT | 2056 | 28,042 | yes | tlm_oev_haltestelle |
| `tlm_eo_einzelobjekt` | POINT | 2056 | 23,749 | yes | tlm_eo_einzelobjekt |
| `tlm_bauten_mauer` | LINESTRING | 2056 | 16,675 | yes | tlm_bauten_mauer |
| `tlm_bauten_sportbaute_ply` | POLYGON | 2056 | 16,641 | yes | tlm_bauten_sportbaute_ply |
| `tlm_name_gelaendename` | POLYGON | 2056 | 15,966 | yes | tlm_name_gelaendename |
| `tlm_areale_verkehrsareal` | POLYGON | 2056 | 15,167 | yes | tlm_areale_verkehrsareal |
| `tlm_namen_name_pkt` | POINT | 2056 | 12,529 | yes | tlm_namen_name_pkt |
| `tlm_bauten_versorgungsbaute_pkt` | POINT | 2056 | 6,831 | yes | tlm_bauten_versorgungsbaute_pkt |
| `tlm_namen_gebietsname` | POLYGON | 2056 | 3,927 | yes | tlm_namen_gebietsname |
| `tlm_areale_freizeitareal` | POLYGON | 2056 | 3,400 | yes | tlm_areale_freizeitareal |
| `tlm_bauten_sportbaute_lin` | LINESTRING | 2056 | 3,160 | yes | tlm_bauten_sportbaute_lin |
| `tlm_bauten_verkehrsbaute_ply` | POLYGON | 2056 | 2,965 | yes | tlm_bauten_verkehrsbaute_ply |
| `tlm_oev_uebrige_bahn` | LINESTRING | 2056 | 2,903 | yes | tlm_oev_uebrige_bahn |
| `tlm_bauten_leitung` | LINESTRING | 2056 | 2,882 | yes | tlm_bauten_leitung |
| `tlm_bauten_verkehrsbaute_lin` | LINESTRING | 2056 | 1,966 | yes | tlm_bauten_verkehrsbaute_lin |
| `tlm_strassen_aus_einfahrt` | POINT | 2056 | 1,915 | yes | tlm_strassen_aus_einfahrt |
| `tlm_oev_schifffahrt` | LINESTRING | 2056 | 27 | yes | tlm_oev_schifffahrt |
| `tlm_areale_schutzgebiet` | POLYGON | 2056 | 2 | yes | tlm_areale_schutzgebiet |

## Non-spatial tables

| Table | Kind | Rows |
|---|---|---:|
| `tlm_areale_nutzungsareal_schule` | attribute | 7,864 |
| `tlm_areale_schule` | attribute | 6,142 |
| `tlm_bauten_leitung_stromtrasse` | attribute | 2,979 |
| `tlm_bauten_stromtrasse` | attribute | 3 |
| `tlm_bb_glamos` | attribute | 2,038 |
| `tlm_strassen_strassenname` | attribute | 173,860 |
| `tlm_strassen_strassenname_strasse` | attribute | 948,833 |
| `tlm_strassen_strassenroute` | attribute | 535 |
| `tlm_strassen_strassenroute_strasse` | attribute | 158,559 |

## Layer detail

### `tlm_bb_einzelbaum`

*tlm_bb_einzelbaum*

**11,494,720** rows · geometry `geom` (POINT, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POINT | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 1,473,071 | 20 | `2016` (1,462,472) · `2018` (1,055,945) · `2020` (1,040,131) · `2013` (885,000) · `2024` (805,452) · `2021` (717,484) · `2025` (716,591) · `2015` (645,520) · `2014` (631,742) · `2017` (626,387) · `2022` (586,728) · `2023` (448,740) · `2012` (259,776) · `2019` (68,362) · `2010` (31,437) · `2026` (27,591) · `2008` (12,200) · `2011` (64) · `1900` (25) · `2002` (2) |
| `erstellung_monat` | MEDIUMINT | 2,939,700 | 12 | `6` (6,090,481) · `3` (384,706) · `5` (289,610) · `2` (275,622) · `8` (238,814) · `7` (233,682) · `1` (215,519) · `10` (194,160) · `9` (186,890) · `4` (163,809) · `12` (149,443) · `11` (132,284) |
| `grund_aenderung` | TEXT(50) | 0 | 3 | `Verbessert` (11,489,744) · `Real` (4,975) · `Restrukturiert` (1) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (11,486,594) · `NDB` (8,126) |
| `herkunft_jahr` | MEDIUMINT | 0 | 22 | `2018` (2,018,381) · `2016` (1,523,734) · `2013` (1,086,742) · `2014` (930,064) · `2023` (911,265) · `2020` (841,037) · `2015` (760,647) · `2017` (626,850) · `2022` (571,950) · `2021` (521,908) · `2019` (517,818) · `2012` (443,688) · `2009` (325,567) · `2024` (165,849) · `2011` (134,826) · `2010` (58,810) · `2003` (33,254) · `2002` (12,891) · `2008` (5,689) · `2025` (3,154) · `2001` (571) · `1900` (25) |
| `herkunft_monat` | MEDIUMINT | 5,544,642 | 7 | `6` (5,815,642) · `3` (59,151) · `7` (41,426) · `5` (33,662) · `9` (158) · `1` (21) · `8` (18) |
| `objektart` | TEXT(50) | 0 | 1 | `Einzelbaum` (11,494,720) |
| `revision_jahr` | MEDIUMINT | 0 | 7 | `2024` (4,092,972) · `2023` (2,912,264) · `2025` (2,221,023) · `2020` (1,962,624) · `2022` (155,819) · `2026` (113,782) · `2019` (36,236) |
| `revision_monat` | MEDIUMINT | 0 | 12 | `6` (4,969,915) · `11` (819,368) · `10` (795,693) · `2` (722,388) · `5` (712,209) · `3` (666,503) · `9` (627,666) · `1` (569,845) · `7` (484,375) · `12` (483,032) · `8` (340,833) · `4` (302,893) |
| `revision_qualitaet` | TEXT(50) | 5,548 | 2 | `Akt` (9,490,305) · `2020_Akt` (1,998,867) |

### `tlm_bauten_gebaeude_footprint`

*tlm_bauten_gebaeude_footprint*

**4,125,163** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 27 | `2016` (1,001,170) · `2015` (857,149) · `2017` (416,347) · `2014` (387,862) · `2012` (338,191) · `2013` (227,785) · `2021` (151,741) · `2019` (123,075) · `2018` (112,641) · `2010` (104,315) · `2024` (101,183) · `2022` (66,109) · `2023` (64,355) · `2020` (55,168) · `2011` (51,490) · `2008` (39,585) · `2009` (13,055) · `2025` (10,661) · `2005` (996) · `2002` (654) · `2004` (597) · `2006` (392) · `2001` (321) · `2003` (306) · `2000` (8) · `1900` (4) · `2007` (3) |
| `erstellung_monat` | MEDIUMINT | 0 | 12 | `6` (1,346,581) · `3` (498,999) · `7` (364,404) · `1` (346,551) · `9` (284,248) · `10` (280,344) · `5` (243,378) · `4` (221,854) · `2` (199,306) · `12` (187,186) · `8` (103,135) · `11` (49,177) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (3,764,909) · `Real` (302,986) · `Restrukturiert` (56,121) · `Uebertragen` (1,147) |
| `herkunft` | TEXT(50) | 0 | 6 | `swisstopo` (3,712,420) · `3D-GebCH_T2016` (187,038) · `3D-GebCH_T2015` (155,883) · `3D-GebCH_T2017` (43,491) · `3D-GebCH_T2014` (22,584) · `3D-GebCH_T2013` (3,747) |
| `herkunft_jahr` | MEDIUMINT | 0 | 17 | `2025` (964,108) · `2021` (773,908) · `2024` (372,135) · `2014` (351,767) · `2022` (332,526) · `2013` (319,860) · `2023` (313,723) · `2015` (153,832) · `2019` (134,724) · `2012` (133,722) · `2016` (109,971) · `2018` (79,890) · `2020` (33,242) · `2011` (30,258) · `2017` (13,815) · `2010` (7,676) · `1900` (6) |
| `herkunft_monat` | MEDIUMINT | 0 | 10 | `6` (3,124,925) · `10` (916,299) · `3` (78,841) · `5` (3,914) · `4` (480) · `7` (407) · `9` (114) · `11` (89) · `8` (88) · `1` (6) |
| `objektart` | TEXT(50) | 0 | 20 | `Gebaeude` (3,657,142) · `Offenes Gebaeude` (332,678) · `Lagertank` (53,585) · `Flugdach` (19,102) · `Im Bau` (15,117) · `Sakrales Gebaeude` (11,746) · `Mauer gross` (10,847) · `Treibhaus` (5,434) · `Hochhaus` (4,734) · `Kapelle` (4,512) · `Sakraler Turm` (4,457) · `Turm` (1,520) · `Verbindungsbruecke` (1,503) · `Unterirdisches Gebaeude` (1,195) · `Hochkamin` (861) · `Mauer gross gedeckt` (575) · `Lueftungsschacht` (101) · `Einhausung` (28) · `Historische Baute` (22) · `Kuehlturm` (4) |
| `revision_jahr` | MEDIUMINT | 0 | 15 | `2024` (1,723,555) · `2025` (1,178,802) · `2023` (492,099) · `2019` (374,005) · `2022` (345,254) · `2020` (10,964) · `2017` (166) · `2018` (121) · `2021` (101) · `2016` (74) · `2014` (11) · `2015` (7) · `2011` (2) · `2012` (1) · `2013` (1) |
| `revision_monat` | MEDIUMINT | 0 | 10 | `6` (3,141,344) · `10` (983,582) · `11` (153) · `1` (40) · `7` (12) · `5` (11) · `4` (10) · `2` (8) · `12` (2) · `3` (1) |
| `revision_qualitaet` | TEXT(50) | 38,342 | 5 | `Akt` (3,157,709) · `2020_Akt` (882,229) · `2018_Aufbau` (46,863) · `TLM_2022_RG_Akt` (19) · `1412_Aufbau` (1) |
| `name` | TEXT(254) | 4,113,059 | >300 | _high cardinality — not enumerated_ |
| `tlm_bauten_name_uuid` | TEXT(38) | 4,113,059 | >300 | _high cardinality — not enumerated_ |
| `nutzung` | TEXT(254) | 4,110,235 | 15 | `Gasthof abgelegen` (5,492) · `Reservoir` (5,056) · `Schiessstand` (2,989) · `Stadion` (692) · `Parkhaus` (206) · `Aussichtsturm` (177) · `Observatorium` (141) · `Schutzhuette` (135) · `Wasserturm` (19) · `Sporthalle` (9) · `Aussichtsturm, Wasserturm` (5) · `Gasthof abgelegen, Schiessstand` (2) · `Leuchtturm` (2) · `Wasserturm, Aussichtsturm` (2) · `Observatorium, Gasthof abgelegen` (1) |

### `tlm_strassen_strasse`

*tlm_strassen_strasse*

**2,096,632** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 5,939 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 40 | 29 | `2010` (213,629) · `2002` (199,537) · `2004` (196,805) · `2005` (170,443) · `2006` (161,541) · `2000` (132,853) · `2019` (113,063) · `2018` (87,953) · `2016` (83,329) · `2009` (74,487) · `2012` (73,683) · `2011` (67,795) · `2015` (61,667) · `2003` (59,199) · `2021` (56,089) · `2020` (53,066) · `2013` (50,773) · `2008` (49,271) · `2014` (46,848) · `2024` (34,338) · `2022` (28,718) · `2023` (24,996) · `2017` (18,172) · `2001` (13,402) · `1999` (8,898) · `1998` (7,777) · `2025` (5,454) · `1900` (2,246) · `2007` (560) |
| `erstellung_monat` | MEDIUMINT | 331,788 | 12 | `6` (741,424) · `4` (598,234) · `3` (161,699) · `8` (109,456) · `7` (76,605) · `5` (63,013) · `9` (9,219) · `1` (3,168) · `10` (997) · `11` (562) · `12` (395) · `2` (72) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (2,025,792) · `Real` (41,175) · `Restrukturiert` (29,646) · `Uebertragen` (19) |
| `herkunft` | TEXT(50) | 0 | 5 | `swisstopo` (2,096,486) · `TLM_Ausnahme` (78) · `SchweizMobil` (34) · `V25` (33) · `LV Kanton` (1) |
| `herkunft_jahr` | MEDIUMINT | 0 | 23 | `2019` (494,034) · `2024` (459,008) · `2021` (398,837) · `2023` (340,769) · `2020` (241,943) · `2022` (112,653) · `2025` (34,747) · `2018` (7,196) · `2017` (5,080) · `2016` (576) · `2010` (350) · `2014` (291) · `2013` (280) · `2011` (260) · `2015` (253) · `2012` (175) · `2009` (95) · `2008` (38) · `2006` (25) · `1900` (13) · `2002` (4) · `2003` (3) · `2000` (2) |
| `herkunft_monat` | MEDIUMINT | 34 | 12 | `6` (2,089,744) · `2` (5,883) · `4` (271) · `3` (236) · `7` (138) · `5` (133) · `8` (72) · `9` (48) · `10` (39) · `11` (15) · `1` (13) · `12` (6) |
| `objektart` | TEXT(50) | 0 | 22 | `2m Weg` (710,496) · `4m Strasse` (425,569) · `3m Strasse` (402,420) · `1m Weg` (337,017) · `6m Strasse` (128,032) · `8m Strasse` (25,473) · `10m Strasse` (16,420) · `Verbindung` (16,236) · `1m Wegfragment` (8,195) · `Markierte Spur` (6,748) · `Autobahn` (6,160) · `Platz` (3,997) · `Einfahrt` (1,838) · `Ausfahrt` (1,825) · `2m Wegfragment` (1,732) · `Dienstzufahrt` (1,621) · `Autostrasse` (1,542) · `Raststaette` (705) · `Zufahrt` (432) · `Klettersteig` (141) · `Faehre` (27) · `Autozug` (6) |
| `revision_jahr` | MEDIUMINT | 27 | 17 | `2024` (843,546) · `2023` (583,479) · `2019` (360,495) · `2025` (186,056) · `2022` (111,319) · `2021` (5,668) · `2020` (5,513) · `2018` (337) · `2014` (63) · `2011` (57) · `2008` (22) · `2015` (18) · `2012` (16) · `1900` (8) · `2009` (6) · `2013` (1) · `2010` (1) |
| `revision_monat` | MEDIUMINT | 27 | 10 | `6` (2,096,407) · `4` (66) · `9` (39) · `8` (37) · `10` (25) · `5` (14) · `1` (8) · `3` (6) · `11` (2) · `7` (1) |
| `revision_qualitaet` | TEXT(50) | 10,797 | 9 | `Akt` (1,652,173) · `2020_Akt` (433,475) · `TLM_GN_Leer` (159) · `2016_Aufbau` (7) · `2021_Akt` (6) · `2018_Aufbau` (6) · `TLM_2022_RG_Akt` (6) · `1412_Aufbau` (2) · `2015_Aufbau` (1) |
| `kunstbaute` | TEXT(30) | 0 | 16 | `Keine` (1,961,749) · `Bruecke` (56,012) · `Treppe` (38,328) · `k_W` (27,155) · `Unterfuehrung` (6,555) · `Tunnel` (1,739) · `Furt` (1,702) · `Bruecke mit Treppe` (876) · `in/auf Gebaeude` (610) · `Galerie` (440) · `Gedeckte Bruecke` (424) · `Unterfuehrung mit Treppe` (415) · `Steg` (334) · `Staumauer, Wehr` (232) · `Staudamm` (43) · `Bruecke mit Galerie` (18) |
| `wanderwege` | TEXT(50) | 1,687,368 | 3 | `Wanderweg` (318,572) · `Bergwanderweg` (87,997) · `Alpinwanderweg` (2,695) |
| `befahrbarkeit` | TEXT(10) | 0 | 3 | `k_W` (983,716) · `Wahr` (939,998) · `Falsch` (172,918) |
| `eroeffnungsdatum` | DATE | 2,096,588 | 9 | `2026-12-31` (14) · `2031-12-31` (12) · `2027-06-30` (5) · `2029-12-31` (3) · `2033-12-31` (3) · `2027-12-31` (3) · `2030-06-30` (2) · `2030-12-31` (1) · `2028-06-30` (1) |
| `stufe` | TEXT(5) | 0 | 11 | `0` (2,029,243) · `1` (57,631) · `-2` (8,333) · `2` (828) · `-1` (470) · `-3` (72) · `3` (29) · `4` (18) · `k_W` (6) · `-5` (1) · `-4` (1) |
| `richtungsgetrennt` | TEXT(10) | 0 | 3 | `Falsch` (1,687,402) · `k_W` (370,102) · `Wahr` (39,128) |
| `tlm_strassen_name_uuid` | TEXT(38) | 2,093,361 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 2,093,361 | >300 | _high cardinality — not enumerated_ |
| `belagsart` | TEXT(10) | 0 | 3 | `Hart` (1,433,191) · `Natur` (640,283) · `k_W` (23,158) |
| `kreisel` | TEXT(10) | 0 | 3 | `k_W` (1,086,298) · `Falsch` (995,996) · `Wahr` (14,338) |
| `verkehrsbeschraenkung` | TEXT(50) | 0 | 11 | `Keine` (1,949,900) · `Allgemeine Verkehrsbeschraenkung` (127,868) · `k_W` (16,404) · `Panzerpiste` (1,541) · `Gesicherte Kletterpartie` (201) · `Zeitlich geregelt` (179) · `Teststrecke` (165) · `Militaerstrasse` (128) · `Gesperrt` (122) · `Rennstrecke` (84) · `Allgemeines Fahrverbot` (40) |
| `eigentuemer` | TEXT(50) | 0 | 4 | `k_W` (1,064,361) · `ub` (825,282) · `Kanton` (182,064) · `Bund` (24,925) |
| `verkehrsbedeutung` | TEXT(50) | 0 | 4 | `k_W` (1,968,608) · `Durchgangsstrasse` (59,840) · `Verbindungsstrasse` (56,387) · `Hochleistungsstrasse` (11,797) |
| `strassenname` | TEXT(254) | 1,150,529 | >300 | _high cardinality — not enumerated_ |

### `tlm_strassen_strasseninfo`

*tlm_strassen_strasseninfo*

**1,654,515** rows · geometry `geom` (POINT, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POINT | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 1 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 21 | `2013` (1,005,922) · `2012` (114,048) · `2019` (91,694) · `2018` (75,108) · `2016` (70,236) · `2015` (51,922) · `2021` (48,226) · `2020` (44,939) · `2014` (40,748) · `2024` (30,079) · `2022` (24,943) · `2023` (21,698) · `2017` (15,458) · `2010` (5,506) · `2025` (4,786) · `2011` (3,248) · `1900` (2,776) · `2009` (1,933) · `2008` (1,200) · `2007` (41) · `2001` (4) |
| `erstellung_monat` | MEDIUMINT | 188 | 12 | `11` (969,247) · `6` (573,744) · `3` (72,555) · `5` (25,132) · `7` (5,112) · `4` (2,848) · `1` (2,776) · `8` (1,382) · `10` (660) · `12` (443) · `9` (393) · `2` (35) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (1,596,632) · `Restrukturiert` (42,205) · `Real` (15,675) · `Uebertragen` (3) |
| `herkunft` | TEXT(50) | 0 | 3 | `swisstopo` (1,654,260) · `SchweizMobil` (254) · `Transportunternehmen` (1) |
| `herkunft_jahr` | MEDIUMINT | 0 | 20 | `2019` (416,053) · `2024` (330,948) · `2020` (270,671) · `2021` (265,995) · `2023` (130,649) · `2022` (102,358) · `2018` (52,796) · `2013` (42,923) · `2025` (9,483) · `2017` (9,426) · `2016` (7,500) · `2015` (5,406) · `2014` (5,164) · `2012` (4,384) · `2010` (271) · `1900` (216) · `2011` (172) · `2009` (57) · `2008` (41) · `2007` (2) |
| `herkunft_monat` | MEDIUMINT | 0 | 12 | `6` (1,601,542) · `11` (40,024) · `2` (7,869) · `3` (3,105) · `5` (829) · `7` (263) · `1` (216) · `10` (192) · `4` (157) · `8` (151) · `12` (111) · `9` (56) |
| `objektart` | TEXT(50) | 0 | 11 | `Standardknoten` (1,611,694) · `Durchfahrtssperre` (23,796) · `Namen` (14,205) · `MISTRA Zusatzknoten` (2,867) · `Loop Junction` (1,596) · `Raststaette` (174) · `Zollamt 24h 24h` (154) · `Verladestation` (15) · `Zollamt 24h eingeschraenkt` (9) · `Zahlstelle` (3) · `Zollamt eingeschraenkt` (2) |
| `revision_jahr` | MEDIUMINT | 0 | 18 | `2024` (660,089) · `2023` (422,606) · `2019` (319,043) · `2025` (128,850) · `2022` (113,317) · `2020` (9,786) · `1900` (354) · `2021` (287) · `2015` (54) · `2014` (51) · `2018` (43) · `2017` (13) · `2011` (8) · `2016` (8) · `2012` (2) · `2013` (2) · `2008` (1) · `2009` (1) |
| `revision_monat` | MEDIUMINT | 0 | 9 | `6` (1,653,720) · `1` (455) · `10` (136) · `8` (117) · `9` (45) · `4` (34) · `11` (4) · `5` (2) · `3` (2) |
| `revision_qualitaet` | TEXT(50) | 22,374 | 9 | `Akt` (1,303,455) · `2020_Akt` (328,640) · `TLM_GN_Leer` (12) · `2016_Aufbau` (9) · `2015_Aufbau` (8) · `2017_Aufbau` (7) · `2018_Aufbau` (6) · `TLM_2022_RG_Akt` (3) · `1412_Aufbau` (1) |
| `tankstelle` | TEXT(10) | 1,654,515 | 0 |  |
| `restaurant` | TEXT(10) | 1,654,515 | 0 |  |
| `shop` | TEXT(10) | 1,654,515 | 0 |  |
| `toiletten` | TEXT(10) | 1,654,515 | 0 |  |
| `tlm_strassen_name_uuid` | TEXT(38) | 1,654,164 | 271 | `{D6AC0A70-A618-48E4-A2F5-18E8A9281AAC}` (6) · `{638F6862-D148-4856-A55E-CC4917619135}` (4) · `{5A25D4E0-6762-4038-B026-C549AF496AC4}` (4) · `{952678CE-DD1E-4F37-A6EF-125A2685D632}` (3) · `{E58AA7A3-E2F3-4E79-B862-F309CF6E9F48}` (2) · `{3EF23C84-26B0-4625-A739-7C63408ED3C0}` (2) · `{91ED14D3-9ABA-437A-AEAB-F23FB33E771F}` (2) · `{D844CEE0-4CE7-48EC-AE95-BE9E22F3AAE8}` (2) · `{C1C52A73-1404-49DB-BE7B-299DBD49DDE1}` (2) · `{265A7B17-279B-4D42-8FBB-FFFF8F7DC8B2}` (2) · `{374FB6A8-C261-4968-81FF-AD3A7636683E}` (2) · `{0970E5BA-4644-41E4-871B-EB9376F4F79C}` (2) · `{25C08D65-F8B5-4D3C-8054-1655863F90AF}` (2) · `{5BFCC239-AE3B-4ECF-BD31-20F7384B6311}` (2) · `{1167CE3F-C5BC-46CF-9239-011B1A614F05}` (2) · `{2EBEAF5C-F837-4C90-887B-60FCA1BE60E9}` (2) · `{6DC51FF9-9360-443D-B33C-299178BA0C10}` (2) · `{681B1A65-41AC-4A96-BD76-78BBF357D7CB}` (2) · `{8C4FB46C-6387-40DE-ADEF-B56B0F5A5BAA}` (2) · `{D461C640-C5B9-4755-B70C-AA6E0DF44B62}` (2) · `{E75EF52C-77C5-479F-8067-8D715A5786F7}` (2) · `{A1A7275B-8C3F-423B-87BB-2119ED23E087}` (2) · `{A046CD5D-914F-4B4C-B14A-2F2387E684FC}` (2) · `{6C267064-332B-4F2F-BA3F-A3A6EC568BDD}` (2) · `{308C1F22-C195-4760-98BC-D33B78065E9E}` (2) · `{FF0FA86D-76CF-434F-A11E-F74986C8E8DF}` (2) · `{DA35FCB3-F9C6-40E4-B7FD-09A142CACB16}` (2) · `{61B8083B-1EDB-4C9F-89B7-CA9040F6D26D}` (2) · `{04264206-EFF2-46D0-BBED-BC950D4A421F}` (2) · `{3C5BB74A-2052-4B89-A990-F384CF4A02DA}` (2) · `{86C9E768-CDE7-4D13-9087-C3D958DCFB06}` (2) · `{8B7B8980-7CD0-4E40-8352-C27D507EE8B5}` (2) · `{9DE978FE-E78B-4385-9508-6298DEB8188A}` (2) · `{E712DE99-7D25-44A4-B244-792CB81999BA}` (2) · `{185556DA-1BEB-461B-B4FE-30983F46B145}` (2) · `{FD920B46-A7E7-43C8-8927-5206E3C799ED}` (2) · `{C599F6C8-1B4F-4434-BE42-15CA21A37906}` (2) · `{5AF371C1-42DB-4B56-B6D4-732075747133}` (2) · `{F6DE9652-233A-409C-8A1B-E8C66336FBF0}` (2) · `{61914776-FE72-429C-9BD6-E68B2B0CED02}` (2) · … +231 more |
| `name` | TEXT(254) | 1,654,164 | 271 | `Thônex-Vallard` (6) · `Stabio-Confine` (4) · `Bardonnex` (4) · `Bargen` (3) · `Kreuzlingen-Autobahn` (2) · `Romanshorn` (2) · `Ponte Faloppia` (2) · `Pizzamiglio` (2) · `Brogeda-Autostrada` (2) · `Anières` (2) · `Croix-de-Rozon` (2) · `Veyrier` (2) · `Moillesulaz` (2) · `La Motta` (2) · `Martina` (2) · `La Drossa` (2) · `Müstair` (2) · `Rose de la Broye` (2) · `Rheinfelden-Autobahn` (2) · `Castasegna` (2) · `Grauholz` (2) · `Münsingen` (2) · `Fillistorf` (2) · `La Gruyère` (2) · `La Joux des Ponts` (2) · `Relais du St-Bernard` (2) · `Chamoson / Ardon` (2) · `Pieterlen` (2) · `Lavorgo` (2) · `Glarnerland` (2) · `Bergsboden Walensee` (2) · `Heidiland` (2) · `Apfelwuhr` (2) · `Ghiffa` (2) · `Coldrerio` (2) · `Segoma` (2) · `Muzzano` (2) · `Gotthard` (2) · `Glooten` (2) · `Isola` (2) · … +231 more |

### `tlm_bb_bodenbedeckung`

*tlm_bb_bodenbedeckung*

**848,065** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 51 | 28 | `2016` (139,732) · `2015` (77,002) · `2014` (72,605) · `2018` (72,189) · `2003` (68,621) · `2013` (56,671) · `2017` (50,642) · `2006` (46,341) · `2004` (39,461) · `2012` (34,036) · `2005` (29,886) · `2002` (25,857) · `2022` (24,831) · `2023` (16,710) · `2019` (16,219) · `2021` (15,106) · `2001` (14,772) · `2024` (12,534) · `1999` (8,669) · `2020` (7,069) · `2000` (5,144) · `2010` (4,082) · `2011` (3,776) · `1998` (1,695) · `2008` (1,603) · `2025` (1,577) · `2009` (1,161) · `1900` (23) |
| `erstellung_monat` | MEDIUMINT | 240,493 | 9 | `6` (568,300) · `3` (12,083) · `5` (11,948) · `7` (9,434) · `4` (2,974) · `8` (1,768) · `9` (1,040) · `1` (24) · `10` (1) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (824,091) · `Restrukturiert` (12,034) · `Real` (11,866) · `Uebertragen` (74) |
| `herkunft` | TEXT(50) | 0 | 5 | `swisstopo` (847,079) · `NDB` (755) · `V25` (140) · `TLM_Ausnahme` (90) · `SchweizMobil` (1) |
| `herkunft_jahr` | MEDIUMINT | 0 | 26 | `2016` (173,883) · `2015` (95,838) · `2018` (92,457) · `2014` (84,683) · `2017` (63,392) · `2022` (61,955) · `2013` (56,265) · `2023` (40,923) · `2024` (40,694) · `2019` (39,962) · `2021` (38,698) · `2012` (29,985) · `2020` (19,903) · `2025` (5,761) · `2011` (1,680) · `2010` (1,404) · `2008` (283) · `2009` (129) · `2006` (77) · `2003` (44) · `2002` (20) · `1900` (17) · `2004` (5) · `2005` (4) · `2001` (2) · `2000` (1) |
| `herkunft_monat` | MEDIUMINT | 154 | 8 | `6` (819,830) · `5` (9,105) · `3` (8,738) · `7` (8,624) · `4` (1,077) · `8` (325) · `9` (195) · `1` (17) |
| `objektart` | TEXT(50) | 0 | 15 | `Gehoelzflaeche` (239,015) · `Fels` (194,603) · `Lockergestein` (130,626) · `Wald` (104,145) · `Lockergestein locker` (38,724) · `Fels locker` (37,539) · `Gebueschwald` (33,413) · `Wald offen` (22,983) · `Stehende Gewaesser` (18,005) · `Feuchtgebiet` (15,835) · `Schneefeld Toteis` (4,524) · `Felsbloecke` (4,143) · `Gletscher` (2,038) · `Felsbloecke locker` (1,341) · `Fliessgewaesser` (1,131) |
| `revision_jahr` | MEDIUMINT | 1 | 12 | `2024` (243,674) · `2023` (228,325) · `2022` (216,439) · `2019` (101,428) · `2025` (55,112) · `2020` (3,068) · `2014` (7) · `2021` (4) · `2012` (3) · `2013` (2) · `2011` (1) · `2018` (1) |
| `revision_monat` | MEDIUMINT | 1 | 4 | `6` (847,996) · `9` (65) · `7` (2) · `3` (1) |
| `revision_qualitaet` | TEXT(50) | 1,150 | 5 | `Akt` (737,546) · `2020_Akt` (109,362) · `TLM_2022_RG_Akt` (5) · `TLM_GN_Leer` (1) · `1312_Akt` (1) |

### `tlm_gewaesser_fliessgewaesser`

*tlm_gewaesser_fliessgewaesser*

**671,334** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 28 | `2016` (102,967) · `2014` (74,349) · `2013` (73,788) · `2015` (66,739) · `2012` (41,576) · `2006` (40,346) · `2018` (38,787) · `2002` (35,634) · `2004` (32,324) · `2003` (32,067) · `2017` (19,065) · `2005` (15,037) · `2021` (13,301) · `2019` (12,378) · `2000` (10,377) · `2023` (10,274) · `2024` (9,507) · `2001` (9,347) · `2022` (9,202) · `2011` (6,017) · `1999` (4,745) · `2010` (4,054) · `2020` (3,795) · `1998` (2,275) · `2008` (1,252) · `2025` (1,195) · `2009` (909) · `1900` (27) |
| `erstellung_monat` | MEDIUMINT | 181,696 | 9 | `6` (442,681) · `5` (14,798) · `3` (13,858) · `7` (10,353) · `4` (4,722) · `8` (2,338) · `9` (854) · `1` (27) · `10` (7) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (654,250) · `Restrukturiert` (12,139) · `Real` (4,943) · `Uebertragen` (2) |
| `herkunft` | TEXT(50) | 0 | 4 | `swisstopo` (671,266) · `V25` (40) · `NDB` (26) · `SchweizMobil` (2) |
| `herkunft_jahr` | MEDIUMINT | 0 | 25 | `2016` (90,420) · `2019` (75,769) · `2021` (67,769) · `2022` (59,922) · `2014` (56,635) · `2015` (55,075) · `2013` (54,590) · `2018` (48,977) · `2024` (45,705) · `2023` (44,017) · `2012` (25,884) · `2020` (23,347) · `2017` (18,824) · `2025` (2,831) · `2011` (798) · `2010` (422) · `2008` (185) · `2009` (106) · `1900` (23) · `2006` (11) · `2003` (7) · `2002` (6) · `2004` (5) · `2000` (3) · `2005` (3) |
| `herkunft_monat` | MEDIUMINT | 31 | 11 | `6` (649,546) · `5` (7,737) · `3` (6,910) · `7` (5,610) · `4` (577) · `8` (574) · `9` (317) · `1` (23) · `2` (5) · `10` (3) · `12` (1) |
| `objektart` | TEXT(50) | 0 | 7 | `Fliessgewaesser` (465,312) · `Trockenrinne` (186,476) · `Seeachse` (11,239) · `Bisse Suone` (6,957) · `Druckstollen` (738) · `Druckleitung einfach` (512) · `Druckleitung mehrfach` (100) |
| `revision_jahr` | MEDIUMINT | 0 | 16 | `2024` (225,169) · `2022` (142,352) · `2023` (132,792) · `2019` (122,620) · `2025` (43,776) · `2020` (4,501) · `2021` (49) · `2018` (46) · `2015` (8) · `2013` (7) · `2009` (5) · `2012` (4) · `2016` (2) · `2011` (1) · `2014` (1) · `1900` (1) |
| `revision_monat` | MEDIUMINT | 0 | 5 | `6` (671,164) · `9` (163) · `8` (5) · `3` (1) · `1` (1) |
| `revision_qualitaet` | TEXT(50) | 4,549 | 8 | `Akt` (533,960) · `2020_Akt` (132,750) · `TLM_GN_Leer` (63) · `1412_Akt` (7) · `2015_Aufbau` (2) · `2017_Aufbau` (1) · `2021_Akt` (1) · `TLM_2022_RG_Akt` (1) |
| `tlm_gewaesser_name_uuid` | TEXT(38) | 553,431 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 553,431 | >300 | _high cardinality — not enumerated_ |
| `verlauf` | TEXT(30) | 0 | 6 | `Oberirdisch` (519,122) · `Unterirdisch bestimmt` (116,370) · `Unterirdisch unbestimmt` (21,462) · `k_W` (11,239) · `Wasserfall` (2,791) · `Bruecke` (350) |
| `tlm_gewaesser_lauf_uuid` | TEXT(38) | 185,671 | >300 | _high cardinality — not enumerated_ |
| `gewiss_nr` | MEDIUMINT | 185,671 | >300 | _high cardinality — not enumerated_ |
| `lauf_nr` | MEDIUMINT | 185,671 | >300 | _high cardinality — not enumerated_ |
| `linst` | TEXT(5) | 185,671 | 27 | `CH` (292,468) · `BE` (42,043) · `SG` (22,369) · `TI` (18,865) · `LU` (17,181) · `ZH` (16,494) · `VD` (13,353) · `SZ` (12,651) · `AG` (9,698) · `TG` (6,056) · `GL` (4,664) · `UR` (4,357) · `OW` (4,279) · `SO` (3,563) · `AR` (3,404) · `BL` (2,915) · `JU` (2,405) · `AI` (2,367) · `FL` (1,721) · `NW` (1,434) · `GR` (1,081) · `SH` (864) · `NE` (777) · `GE` (633) · `BS` (13) · `VS` (7) · `FR` (1) |
| `gwl_nr` | TEXT(50) | 185,671 | >300 | _high cardinality — not enumerated_ |
| `stufe` | TEXT(5) | 0 | 10 | `0` (519,122) · `-1` (135,948) · `k_W` (11,239) · `1` (3,132) · `-2` (1,771) · `-3` (102) · `2` (9) · `-4` (8) · `-6` (2) · `-5` (1) |

### `tlm_namen_flurname`

*tlm_namen_flurname*

**279,477** rows · geometry `geom` (POINT, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POINT | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 17 | `2012` (49,763) · `2016` (36,722) · `2013` (31,064) · `2015` (28,733) · `2014` (26,952) · `2018` (16,564) · `2008` (15,857) · `2009` (13,378) · `2023` (10,525) · `2024` (9,931) · `2010` (8,988) · `2019` (8,884) · `2017` (7,999) · `2025` (7,016) · `2022` (3,628) · `2020` (3,192) · `2011` (281) |
| `erstellung_monat` | MEDIUMINT | 114,755 | 12 | `6` (157,730) · `3` (3,457) · `5` (2,953) · `10` (169) · `12` (140) · `9` (73) · `11` (61) · `8` (47) · `1` (38) · `4` (26) · `2` (23) · `7` (5) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (224,567) · `Uebertragen` (50,139) · `Real` (2,677) · `Restrukturiert` (2,094) |
| `herkunft` | TEXT(50) | 0 | 3 | `swisstopo` (228,058) · `AV` (50,453) · `NDB` (966) |
| `herkunft_jahr` | MEDIUMINT | 0 | 17 | `2024` (102,652) · `2025` (45,465) · `2023` (22,284) · `2012` (17,603) · `2013` (17,132) · `2016` (13,317) · `2018` (12,285) · `2008` (10,678) · `2015` (10,422) · `2014` (7,919) · `2019` (4,484) · `2017` (4,386) · `2011` (3,071) · `2009` (2,848) · `2020` (2,048) · `2022` (1,965) · `2010` (918) |
| `herkunft_monat` | MEDIUMINT | 41,514 | 11 | `6` (232,512) · `1` (3,027) · `3` (1,959) · `11` (174) · `5` (172) · `12` (61) · `10` (37) · `9` (9) · `2` (8) · `4` (2) · `8` (2) |
| `objektart` | TEXT(50) | 0 | 2 | `Flurname swisstopo` (261,528) · `Lokalname swisstopo` (17,949) |
| `revision_jahr` | MEDIUMINT | 0 | 1 | `2025` (279,477) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (279,477) |
| `revision_qualitaet` | TEXT(50) | 0 | 1 | `TGMG_2020_Akt` (279,477) |
| `tlm_namen_name_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 0 | >300 | _high cardinality — not enumerated_ |

### `tlm_bauten_verbauung`

*tlm_bauten_verbauung*

**83,109** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 22 | `1900` (22,891) · `2011` (8,914) · `2012` (8,375) · `2024` (5,906) · `2023` (5,733) · `2013` (5,657) · `2016` (5,599) · `2022` (4,658) · `2014` (3,593) · `2015` (2,310) · `2010` (2,294) · `2019` (1,806) · `2018` (1,725) · `2017` (1,340) · `2021` (1,330) · `2020` (712) · `2025` (179) · `2009` (57) · `2008` (26) · `2003` (2) · `2004` (1) · `2006` (1) |
| `erstellung_monat` | MEDIUMINT | 4 | 8 | `6` (49,198) · `1` (22,891) · `3` (3,497) · `7` (3,219) · `5` (2,161) · `8` (2,043) · `9` (63) · `4` (33) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (74,436) · `Uebertragen` (6,711) · `Real` (1,015) · `Restrukturiert` (947) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (83,106) · `V25` (3) |
| `herkunft_jahr` | MEDIUMINT | 0 | 22 | `2023` (9,994) · `2022` (8,831) · `2024` (8,055) · `2011` (7,841) · `2012` (7,049) · `2016` (6,851) · `2013` (5,376) · `2015` (3,744) · `2014` (3,624) · `2010` (2,866) · `2018` (2,822) · `2019` (2,747) · `2021` (2,683) · `2017` (2,569) · `2007` (1,798) · `2008` (1,687) · `2025` (1,385) · `2020` (1,231) · `2006` (1,128) · `2009` (579) · `2005` (233) · `2001` (16) |
| `herkunft_monat` | MEDIUMINT | 0 | 8 | `6` (67,174) · `1` (6,730) · `3` (3,051) · `7` (2,416) · `8` (1,857) · `5` (1,821) · `9` (55) · `4` (5) |
| `objektart` | TEXT(50) | 0 | 3 | `Schutzverbauung` (38,485) · `Gewaesserverbauung` (28,029) · `Trockenmauer` (16,595) |
| `revision_jahr` | MEDIUMINT | 0 | 6 | `2023` (25,804) · `2024` (24,495) · `2022` (17,493) · `2019` (9,180) · `2025` (5,817) · `2020` (320) |
| `revision_monat` | MEDIUMINT | 0 | 2 | `6` (83,108) · `9` (1) |
| `revision_qualitaet` | TEXT(50) | 767 | 2 | `Akt` (72,511) · `2020_Akt` (9,831) |

### `tlm_bauten_staubaute`

*tlm_bauten_staubaute*

**73,219** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 28 | `2015` (10,111) · `2018` (8,276) · `2016` (7,411) · `2021` (6,943) · `2019` (6,254) · `2024` (5,876) · `2013` (5,503) · `2014` (5,501) · `2023` (4,118) · `2020` (4,095) · `2017` (2,314) · `2012` (1,912) · `2022` (1,288) · `2010` (1,018) · `2025` (723) · `2000` (689) · `2002` (327) · `1998` (291) · `2011` (97) · `2001` (89) · `2006` (75) · `2005` (63) · `2003` (61) · `1997` (60) · `2004` (40) · `1999` (34) · `2008` (28) · `2009` (22) |
| `erstellung_monat` | MEDIUMINT | 1,729 | 6 | `6` (68,362) · `3` (1,482) · `5` (776) · `4` (564) · `7` (295) · `8` (11) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (66,983) · `Real` (6,113) · `Restrukturiert` (114) · `Uebertragen` (9) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (73,158) · `NDB` (61) |
| `herkunft_jahr` | MEDIUMINT | 0 | 18 | `2015` (9,819) · `2018` (9,098) · `2019` (7,338) · `2021` (7,321) · `2016` (7,085) · `2024` (6,232) · `2013` (5,180) · `2014` (4,966) · `2020` (4,611) · `2023` (4,522) · `2017` (2,425) · `2022` (1,408) · `2012` (1,275) · `2010` (1,059) · `2025` (820) · `2011` (42) · `2008` (14) · `2009` (4) |
| `herkunft_monat` | MEDIUMINT | 0 | 6 | `6` (70,743) · `3` (864) · `5` (701) · `4` (572) · `7` (338) · `8` (1) |
| `objektart` | TEXT(50) | 0 | 5 | `Wasserbecken` (71,164) · `Schutzdamm` (1,472) · `Wehr` (325) · `Staumauer` (153) · `Staudamm` (105) |
| `revision_jahr` | MEDIUMINT | 0 | 7 | `2024` (30,612) · `2023` (23,728) · `2019` (11,633) · `2025` (5,031) · `2022` (1,875) · `2020` (330) · `2021` (10) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (73,219) |
| `revision_qualitaet` | TEXT(50) | 729 | 3 | `Akt` (60,487) · `2020_Akt` (12,002) · `2018_Aufbau` (1) |
| `tlm_bauten_name_uuid` | TEXT(38) | 72,881 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 72,881 | >300 | _high cardinality — not enumerated_ |

### `tlm_oev_eisenbahn`

*tlm_oev_eisenbahn*

**56,026** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 27 | `2010` (18,921) · `2009` (15,562) · `2008` (8,351) · `2011` (4,135) · `2024` (1,350) · `2016` (1,003) · `2019` (834) · `2021` (719) · `2023` (688) · `2018` (614) · `2015` (577) · `2012` (551) · `2020` (504) · `2022` (478) · `2014` (460) · `2025` (426) · `2013` (344) · `2017` (159) · `2007` (112) · `2004` (77) · `2002` (63) · `2001` (41) · `1998` (25) · `1999` (18) · `2005` (8) · `2006` (5) · `2000` (1) |
| `erstellung_monat` | MEDIUMINT | 238 | 10 | `6` (25,254) · `7` (6,686) · `4` (6,494) · `5` (5,846) · `3` (4,980) · `8` (4,448) · `9` (2,049) · `10` (25) · `12` (4) · `11` (2) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (51,625) · `Real` (2,870) · `Restrukturiert` (1,530) · `Uebertragen` (1) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (56,024) · `Transportunternehmen` (2) |
| `herkunft_jahr` | MEDIUMINT | 0 | 18 | `2024` (15,626) · `2019` (12,539) · `2021` (9,557) · `2020` (7,701) · `2023` (4,504) · `2022` (3,239) · `2025` (1,806) · `2018` (207) · `2010` (168) · `2009` (157) · `2017` (120) · `2011` (113) · `2016` (103) · `2014` (54) · `2008` (37) · `2015` (36) · `2012` (33) · `2013` (26) |
| `herkunft_monat` | MEDIUMINT | 0 | 9 | `6` (55,236) · `2` (359) · `3` (122) · `9` (112) · `4` (65) · `10` (43) · `7` (35) · `5` (29) · `8` (25) |
| `objektart` | TEXT(50) | 0 | 4 | `Normalspur` (43,535) · `Schmalspur` (12,126) · `Schmalspur mit Normalspur` (241) · `Kleinbahn` (124) |
| `revision_jahr` | MEDIUMINT | 0 | 10 | `2024` (24,699) · `2023` (12,351) · `2019` (9,422) · `2025` (5,532) · `2022` (3,519) · `2020` (462) · `2018` (19) · `2011` (13) · `2014` (6) · `2016` (3) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (56,026) |
| `revision_qualitaet` | TEXT(50) | 280 | 3 | `Akt` (44,323) · `2020_Akt` (11,422) · `1412_Aufbau` (1) |
| `kunstbaute` | TEXT(30) | 0 | 7 | `Keine` (46,239) · `Bruecke` (6,759) · `Tunnel` (1,465) · `in/auf Gebaeude` (1,243) · `Unterfuehrung` (177) · `Galerie` (129) · `Gedeckte Bruecke` (14) |
| `anschlussgleis` | TEXT(10) | 0 | 2 | `Falsch` (35,878) · `Wahr` (20,148) |
| `achse_dkm` | TEXT(10) | 0 | 2 | `Falsch` (39,520) · `Wahr` (16,506) |
| `museumsbahn` | TEXT(10) | 0 | 2 | `Falsch` (55,852) · `Wahr` (174) |
| `auf_strasse` | TEXT(10) | 0 | 2 | `Falsch` (51,364) · `Wahr` (4,662) |
| `eroeffnungsdatum` | DATE | 55,998 | 4 | `2027-12-31` (12) · `2029-12-31` (11) · `2026-12-31` (3) · `2029-12-16` (2) |
| `stufe` | TEXT(5) | 0 | 8 | `0` (47,571) · `1` (6,588) · `-2` (1,595) · `2` (190) · `-3` (48) · `-1` (24) · `3` (7) · `-4` (3) |
| `ausser_betrieb` | TEXT(10) | 0 | 2 | `Falsch` (55,824) · `Wahr` (202) |
| `zahnradbahn` | TEXT(10) | 0 | 2 | `Falsch` (54,925) · `Wahr` (1,101) |
| `standseilbahn` | TEXT(10) | 0 | 2 | `Falsch` (55,401) · `Wahr` (625) |
| `betriebsbahn` | TEXT(10) | 0 | 2 | `Falsch` (55,892) · `Wahr` (134) |
| `tlm_oev_name_uuid` | TEXT(38) | 55,500 | 179 | `{73F7A42B-EE38-429F-B454-243EB5B509A2}` (29) · `{912E95CE-8A83-442E-B5B9-73E8687E5F44}` (25) · `{CAF697E6-9507-4F1A-941F-B785CF1E7E1E}` (25) · `{9F3D4D16-DF49-4E85-B4CA-66EE5DABA02A}` (18) · `{BD737A13-EF6C-47E4-B036-39127BE37FBF}` (15) · `{F669CCF1-9D51-4912-9121-B3C0B00F2F52}` (14) · `{2E3798A7-B5F4-498A-8166-663E045E5A69}` (13) · `{F3A831AC-2702-4717-AC5E-78535BF9DD4E}` (13) · `{D401945F-0072-4856-87FB-E8384DD43AFC}` (13) · `{5AA87D92-3CC3-4A29-B80B-609D76157F31}` (12) · `{0B36B50D-7D73-4A3F-97DF-409701BE8041}` (12) · `{5CD7B750-6516-47DF-83D0-FFF2FEA614C6}` (9) · `{DC34A21B-972F-420C-9C0E-D6998EADC76A}` (8) · `{C014E4FE-218E-45EC-9179-F9275E6A6DF5}` (8) · `{1043D111-578A-4259-A004-117CDA10B5D3}` (8) · `{888FD80E-25D1-4CA6-95BC-F1515DF67D98}` (8) · `{864676FF-6ACA-43AA-AE26-A83D4689AB0E}` (7) · `{6CAFAD3D-8C63-40BA-A1BA-66665EE47CE9}` (7) · `{5736A10F-9DA4-4116-959B-2A47586B112D}` (7) · `{61A849C0-C7DF-4294-B820-3A868ADF98BA}` (7) · `{F1B4F81C-6678-4CBB-A406-4B6D712B509F}` (7) · `{D1B4180E-B58F-45FA-8AE0-BAEB11B16AEE}` (7) · `{A4FAA485-B38E-4E56-90B4-F83D5A1EB7C2}` (7) · `{B0B2183E-3B8D-4440-9F00-51ACD72C80D0}` (6) · `{95495F53-DD22-4618-9EC4-F75C0BF36523}` (5) · `{F93814A0-B97C-4863-98FC-36D2E53EBCD5}` (5) · `{101C292D-DF2F-4CA8-B4F2-624585CD4AC9}` (5) · `{ED2D9245-EC20-45AF-910E-D1117DF5DEC0}` (5) · `{4EB013EA-49BE-4312-9287-A5B60EF8E552}` (5) · `{CA41B722-6F2E-4650-8CF9-1A91E655149D}` (4) · `{4F399286-7CF3-4152-A187-E2DEE4566B2E}` (4) · `{CB22CF6D-77C4-49E0-9DD7-B701828BC376}` (3) · `{1C365B2C-7111-4278-AEB2-5B974487F4BC}` (3) · `{C3B0E635-3B25-4E32-B53F-DAA6859318EA}` (3) · `{8DA928BB-DBDB-4ACB-A44B-963BB3758DE1}` (3) · `{A636B4E7-1903-4B6D-AC2A-FEC3FB400E4C}` (3) · `{9CCB80B4-BB1D-43BD-93C7-28AEA41782D2}` (3) · `{1E10CA55-FFFA-4664-A916-73F7583B46BE}` (3) · `{00C41DF0-6EC1-4AD8-AE1E-9ADA6C8FFEA4}` (3) · `{85EAE980-FBC6-4A44-952A-4D489F656146}` (2) · … +139 more |
| `name` | TEXT(254) | 55,500 | 178 | `Tunnel dal Veraina | Vereinatunnel` (29) · `Hauenstein-Basistunnel` (25) · `Galleria ferroviaria del San Gottardo | Gotthard-Bahntunnel` (25) · `Zürichbergtunnel` (18) · `Hirschengrabentunnel` (15) · `Galleria di base del San Gottardo | Gotthard-Basistunnel | Tunnel da basa dal Son Gottard` (14) · `Lötschbergtunnel` (13) · `Zimmerberg-Basistunnel` (13) · `Weinbergtunnel` (13) · `Wabern-Gurten` (12) · `Jungfraubahntunnel` (12) · `Reichenbachfall-Bahn` (9) · `Galleria di Base del Ceneri` (8) · `Simplontunnel` (8) · `Schanzentunnel` (8) · `Wipkingertunnel` (8) · `Rosenbergtunnel` (7) · `Lötschberg-Basistunnel` (7) · `Engelbergtunnel` (7) · `Heitersbergtunnel` (7) · `Hagenholztunnel` (7) · `Flughafentunnel` (7) · `Parkbahn Letten` (7) · `Adlertunnel` (6) · `Leuk-Tunnel` (5) · `Tunnel de St-Aubin-Sauges` (5) · `Furka-Basistunnel` (5) · `Albulatunnel` (5) · `Bözbergtunnel` (5) · `Tunnel de Pinchat` (4) · `Tunnel de la Raisse` (4) · `Gelmerbahn` (3) · `Bruggwaldtunnel` (3) · `Tunnel Olivier Français (Tridel)` (3) · `Galleria Lunga` (3) · `Morschachtunnel` (3) · `Albistunnel` (3) · `Riesbachtunnel` (3) · `Skymetro` (3) · `Kerenzerbergtunnel` (2) · … +138 more |
| `anzahl_spuren` | TEXT(10) | 39,520 | 2 | `1` (8,808) · `2` (7,698) |
| `verkehrsmittel` | TEXT(50) | 0 | 3 | `Bahn` (53,146) · `Tram` (2,741) · `Metro` (139) |

### `tlm_namen_siedlungsname`

*tlm_namen_siedlungsname*

**54,885** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 18 | `2009` (27,744) · `2008` (6,309) · `2016` (3,556) · `2014` (2,955) · `2018` (2,600) · `2015` (1,993) · `2013` (1,568) · `2019` (1,437) · `2012` (1,363) · `2011` (1,000) · `2017` (881) · `2020` (846) · `2010` (764) · `2025` (522) · `2023` (503) · `2022` (458) · `2024` (347) · `2007` (39) |
| `erstellung_monat` | MEDIUMINT | 29,288 | 11 | `6` (17,380) · `8` (4,081) · `5` (1,032) · `10` (875) · `11` (590) · `2` (480) · `9` (465) · `12` (449) · `3` (120) · `7` (68) · `1` (57) |
| `grund_aenderung` | TEXT(50) | 0 | 2 | `Verbessert` (54,877) · `Restrukturiert` (8) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (54,885) |
| `herkunft_jahr` | MEDIUMINT | 0 | 13 | `2025` (20,771) · `2023` (19,011) · `2019` (4,133) · `2024` (3,823) · `2018` (3,800) · `2020` (2,342) · `2022` (409) · `2016` (303) · `2014` (208) · `2015` (44) · `2017` (27) · `2013` (8) · `2011` (6) |
| `herkunft_monat` | MEDIUMINT | 0 | 3 | `6` (54,876) · `10` (6) · `11` (3) |
| `objektart` | TEXT(50) | 0 | 4 | `Ort` (34,254) · `Quartierteil` (18,409) · `Quartier` (1,751) · `Ortsteil` (471) |
| `revision_jahr` | MEDIUMINT | 0 | 1 | `2025` (54,885) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (54,885) |
| `revision_qualitaet` | TEXT(50) | 0 | 1 | `TGMG_2020_Akt` (54,885) |
| `tlm_namen_name_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 0 | >300 | _high cardinality — not enumerated_ |
| `einwohnerkategorie` | TEXT(50) | 0 | 9 | `< 20` (36,793) · `100 bis 999` (6,500) · `20 bis 49` (5,596) · `50 bis 99` (3,357) · `2'000 bis 9'999` (1,349) · `1'000 bis 1'999` (1,065) · `10'000 bis 49'999` (209) · `50'000 bis 100'000` (9) · `> 100'000` (7) |

### `tlm_namen_siedlungsname_zentrum`

*tlm_namen_siedlungsname_zentrum*

**54,885** rows · geometry `geom` (POINT, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POINT | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | 49 | `2025-07-15` (53,975) · `2025-10-14` (101) · `2025-11-04` (57) · `2025-12-03` (48) · `2025-12-16` (47) · `2025-09-15` (42) · `2025-10-23` (42) · `2025-10-16` (39) · `2025-10-15` (35) · `2025-12-15` (32) · `2025-12-01` (27) · `2025-10-21` (26) · `2025-10-24` (26) · `2025-12-10` (25) · `2025-10-22` (25) · `2025-10-30` (24) · `2025-12-12` (21) · `2025-10-03` (21) · `2025-09-25` (20) · `2025-10-10` (20) · `2025-11-24` (19) · `2025-12-11` (19) · `2025-10-09` (18) · `2025-12-04` (17) · `2025-11-27` (14) · `2025-09-26` (13) · `2025-10-08` (13) · `2025-12-09` (12) · `2025-09-16` (12) · `2025-09-22` (12) · `2025-09-29` (11) · `2025-10-29` (8) · `2025-10-27` (7) · `2025-09-24` (7) · `2025-12-05` (6) · `2025-10-01` (6) · `2025-10-17` (5) · `2025-10-02` (5) · `2025-09-30` (4) · `2025-11-05` (4) · … +9 more |
| `datum_erstellung` | DATE | 0 | 38 | `2025-07-15` (54,357) · `2025-11-04` (57) · `2025-10-14` (52) · `2025-10-13` (42) · `2025-10-23` (37) · `2025-10-16` (36) · `2025-10-15` (34) · `2025-09-15` (28) · `2025-10-30` (22) · `2025-10-03` (21) · `2025-10-22` (20) · `2025-11-24` (15) · `2025-10-21` (14) · `2025-10-24` (14) · `2025-11-27` (14) · `2025-10-08` (13) · `2025-10-09` (12) · `2025-09-29` (11) · `2025-09-22` (10) · `2025-09-25` (10) · `2025-09-26` (10) · `2025-10-01` (7) · `2025-10-29` (7) · `2025-12-01` (7) · `2025-09-24` (6) · `2025-10-02` (5) · `2025-10-27` (4) · `2025-09-19` (3) · `2025-10-17` (3) · `2025-12-04` (3) · `2025-10-10` (2) · `2025-12-05` (2) · `2025-12-11` (2) · `2025-09-16` (1) · `2025-10-07` (1) · `2025-11-05` (1) · `2025-12-03` (1) · `2025-12-16` (1) |
| `erstellung_jahr` | MEDIUMINT | 0 | 1 | `2025` (54,885) |
| `erstellung_monat` | MEDIUMINT | 0 | 2 | `7` (54,053) · `6` (832) |
| `grund_aenderung` | TEXT(50) | 0 | 2 | `Uebertragen` (53,975) · `Verbessert` (910) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (54,885) |
| `herkunft_jahr` | MEDIUMINT | 0 | 1 | `2025` (54,885) |
| `herkunft_monat` | MEDIUMINT | 0 | 2 | `7` (53,975) · `6` (910) |
| `objektart` | TEXT(50) | 0 | 4 | `Ort` (34,254) · `Quartierteil` (18,409) · `Quartier` (1,751) · `Ortsteil` (471) |
| `revision_jahr` | MEDIUMINT | 0 | 1 | `2025` (54,885) |
| `revision_monat` | MEDIUMINT | 0 | 2 | `7` (53,975) · `6` (910) |
| `revision_qualitaet` | TEXT(50) | 0 | 1 | `TGMG_2020_Akt` (54,885) |
| `tlm_namen_name_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 0 | >300 | _high cardinality — not enumerated_ |
| `einwohnerkategorie` | TEXT(50) | 0 | 9 | `< 20` (36,793) · `100 bis 999` (6,500) · `20 bis 49` (5,596) · `50 bis 99` (3,357) · `2'000 bis 9'999` (1,349) · `1'000 bis 1'999` (1,065) · `10'000 bis 49'999` (209) · `50'000 bis 100'000` (9) · `> 100'000` (7) |
| `tlm_siedlungsname_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |

### `tlm_gewaesser_stehendes_gewaesser`

*tlm_gewaesser_stehendes_gewaesser*

**43,204** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 31 | `2003` (5,796) · `2013` (4,075) · `2006` (4,000) · `2002` (3,876) · `2004` (3,365) · `2005` (2,358) · `2012` (2,227) · `2011` (2,205) · `2014` (1,984) · `2000` (1,758) · `2001` (1,353) · `2015` (1,134) · `2010` (1,018) · `2019` (959) · `2016` (919) · `2018` (904) · `2021` (785) · `1999` (749) · `2023` (657) · `2022` (638) · `2024` (557) · `2008` (521) · `2020` (353) · `2009` (349) · `2017` (311) · `2025` (193) · `1998` (153) · `1984` (2) · `1988` (2) · `1900` (2) · `0` (1) |
| `erstellung_monat` | MEDIUMINT | 23,209 | 8 | `6` (13,789) · `3` (2,397) · `4` (1,332) · `5` (1,127) · `7` (498) · `8` (463) · `9` (377) · `1` (12) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (38,168) · `Restrukturiert` (2,674) · `Real` (2,360) · `Uebertragen` (2) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (43,204) |
| `herkunft_jahr` | MEDIUMINT | 0 | 20 | `2013` (9,389) · `2012` (5,866) · `2014` (4,534) · `2019` (2,948) · `2011` (2,749) · `2022` (2,461) · `2015` (2,061) · `2018` (2,055) · `2023` (1,856) · `2016` (1,817) · `2021` (1,701) · `2024` (1,581) · `2017` (1,322) · `2010` (1,074) · `2020` (772) · `2008` (382) · `2025` (380) · `2009` (244) · `2006` (8) · `2001` (4) |
| `herkunft_monat` | MEDIUMINT | 0 | 8 | `6` (35,119) · `3` (3,138) · `4` (1,717) · `5` (1,605) · `7` (1,064) · `8` (441) · `9` (112) · `1` (8) |
| `objektart` | TEXT(50) | 0 | 2 | `See` (40,998) · `Seeinsel` (2,206) |
| `revision_jahr` | MEDIUMINT | 8 | 14 | `2024` (12,408) · `2023` (10,739) · `2022` (8,592) · `2019` (6,602) · `2025` (4,587) · `2020` (183) · `2014` (29) · `2015` (22) · `2021` (10) · `2012` (9) · `2017` (5) · `2008` (4) · `2016` (4) · `2011` (2) |
| `revision_monat` | MEDIUMINT | 8 | 5 | `6` (43,172) · `9` (12) · `7` (5) · `5` (4) · `3` (3) |
| `revision_qualitaet` | TEXT(50) | 197 | 3 | `Akt` (35,854) · `2020_Akt` (7,129) · `TLM_GN_Leer` (24) |
| `tlm_gewaesser_name_uuid` | TEXT(38) | 36,161 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 36,161 | >300 | _high cardinality — not enumerated_ |
| `tlm_gewaesser_lauf_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `gewiss_nr` | MEDIUMINT | 0 | >300 | _high cardinality — not enumerated_ |
| `lauf_nr` | MEDIUMINT | 0 | >300 | _high cardinality — not enumerated_ |
| `linst` | TEXT(5) | 0 | 16 | `CH` (42,194) · `GL` (284) · `SZ` (222) · `JU` (187) · `BL` (126) · `FL` (81) · `SG` (39) · `SO` (16) · `NW` (14) · `AG` (11) · `ZH` (7) · `LU` (6) · `GR` (6) · `VD` (4) · `TI` (4) · `BE` (3) |
| `gwl_nr` | TEXT(50) | 0 | >300 | _high cardinality — not enumerated_ |
| `wasserstand_wechselnd` | TEXT(50) | 0 | 2 | `Falsch` (33,656) · `Wahr` (9,548) |

### `tlm_areale_nutzungsareal`

*tlm_areale_nutzungsareal*

**36,368** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 29 | `2016` (7,578) · `2013` (2,574) · `2010` (2,502) · `2012` (2,062) · `2011` (1,888) · `2004` (1,871) · `2022` (1,487) · `2002` (1,385) · `2014` (1,353) · `2024` (1,342) · `2015` (1,332) · `2023` (1,292) · `2021` (1,279) · `2018` (1,210) · `2019` (1,208) · `2006` (1,193) · `2008` (930) · `2001` (699) · `2000` (685) · `2005` (642) · `2020` (622) · `2009` (515) · `2017` (340) · `2025` (237) · `2003` (105) · `1998` (23) · `1999` (11) · `1900` (2) · `1997` (1) |
| `erstellung_monat` | MEDIUMINT | 6,615 | 8 | `6` (18,074) · `4` (7,221) · `3` (1,711) · `5` (1,410) · `7` (803) · `8` (431) · `9` (101) · `1` (2) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (32,485) · `Real` (3,044) · `Restrukturiert` (836) · `Uebertragen` (3) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (36,349) · `NDB` (19) |
| `herkunft_jahr` | MEDIUMINT | 0 | 18 | `2016` (4,285) · `2022` (4,241) · `2024` (4,009) · `2021` (3,921) · `2023` (3,396) · `2019` (3,375) · `2018` (2,752) · `2020` (2,578) · `2015` (1,930) · `2014` (1,764) · `2013` (1,261) · `2017` (1,086) · `2025` (709) · `2012` (506) · `2010` (354) · `2011` (125) · `2008` (60) · `2009` (16) |
| `herkunft_monat` | MEDIUMINT | 0 | 7 | `6` (34,769) · `4` (885) · `3` (413) · `5` (150) · `7` (131) · `8` (13) · `9` (7) |
| `name` | TEXT(254) | 29,213 | >300 | _high cardinality — not enumerated_ |
| `objektart` | TEXT(50) | 0 | 21 | `Reben` (7,817) · `Schul- und Hochschulareal` (6,576) · `Wald nicht bestockt` (5,577) · `Obstanlage` (4,525) · `Friedhof` (2,854) · `Baumschule` (2,218) · `Historisches Areal` (1,312) · `Schrebergartenareal` (1,294) · `Oeffentliches Parkareal` (863) · `Kraftwerkareal` (712) · `Abwasserreinigungsareal` (680) · `Abbauareal` (504) · `Unterwerkareal` (493) · `Spitalareal` (392) · `Deponieareal` (157) · `Truppenuebungsplatz` (141) · `Klosterareal` (82) · `Massnahmenvollzugsanstaltsareal` (67) · `Antennenareal` (53) · `Kehrichtverbrennungsareal` (30) · `Messeareal` (21) |
| `revision_jahr` | MEDIUMINT | 0 | 7 | `2024` (13,195) · `2023` (10,629) · `2022` (5,066) · `2019` (4,173) · `2025` (3,028) · `2020` (248) · `2021` (29) |
| `revision_monat` | MEDIUMINT | 0 | 3 | `6` (36,360) · `9` (7) · `4` (1) |
| `revision_qualitaet` | TEXT(50) | 518 | 2 | `Akt` (29,906) · `2020_Akt` (5,944) |
| `tlm_areale_name_uuid` | TEXT(38) | 29,213 | >300 | _high cardinality — not enumerated_ |
| `ris_id` | TEXT(50) | 45 | >300 | _high cardinality — not enumerated_ |

### `tlm_oev_haltestelle`

*tlm_oev_haltestelle*

**28,042** rows · geometry `geom` (POINT, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POINT | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 28 | `2014` (19,952) · `2010` (1,439) · `2009` (1,223) · `2021` (723) · `2015` (673) · `2018` (660) · `2024` (634) · `2008` (335) · `2019` (325) · `2023` (288) · `2025` (268) · `2020` (245) · `2004` (223) · `2017` (190) · `2002` (152) · `2016` (145) · `2011` (135) · `2013` (86) · `2000` (78) · `2012` (77) · `2005` (50) · `2001` (42) · `2022` (39) · `2006` (23) · `1998` (16) · `2003` (16) · `2007` (4) · `1900` (1) |
| `erstellung_monat` | MEDIUMINT | 600 | 9 | `10` (19,714) · `6` (5,796) · `5` (600) · `4` (313) · `3` (306) · `7` (278) · `8` (268) · `9` (166) · `1` (1) |
| `grund_aenderung` | TEXT(50) | 0 | 3 | `Verbessert` (23,650) · `Real` (4,389) · `Uebertragen` (3) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (28,025) · `Transportunternehmen` (17) |
| `herkunft_jahr` | MEDIUMINT | 0 | 18 | `2013` (10,617) · `2024` (5,668) · `2019` (1,730) · `2021` (1,608) · `2020` (1,401) · `2023` (1,328) · `2018` (985) · `2015` (794) · `2010` (655) · `2022` (639) · `2025` (629) · `2016` (602) · `2017` (495) · `2014` (486) · `2009` (208) · `2008` (115) · `2012` (44) · `2011` (38) |
| `herkunft_monat` | MEDIUMINT | 0 | 11 | `6` (16,997) · `12` (10,509) · `5` (116) · `7` (116) · `4` (110) · `8` (72) · `9` (55) · `3` (34) · `2` (25) · `10` (7) · `11` (1) |
| `objektart` | TEXT(50) | 0 | 4 | `Haltestelle Bus` (23,331) · `Haltestelle Bahn` (3,157) · `Uebrige Bahnen` (1,191) · `Haltestelle Schiff` (363) |
| `revision_jahr` | MEDIUMINT | 0 | 9 | `2024` (10,639) · `2023` (7,755) · `2019` (4,939) · `2025` (2,183) · `2022` (2,083) · `2020` (242) · `2021` (196) · `2018` (4) · `2016` (1) |
| `revision_monat` | MEDIUMINT | 0 | 2 | `6` (28,040) · `10` (2) |
| `revision_qualitaet` | TEXT(50) | 689 | 2 | `Akt` (21,796) · `2020_Akt` (5,557) |
| `tlm_oev_name_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 0 | >300 | _high cardinality — not enumerated_ |
| `dienststellen_nummer` | MEDIUMINT | 43 | >300 | _high cardinality — not enumerated_ |

### `tlm_eo_einzelobjekt`

*tlm_eo_einzelobjekt*

**23,749** rows · geometry `geom` (POINT, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POINT | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 28 | `2019` (7,033) · `2006` (2,049) · `2002` (1,717) · `2005` (1,636) · `2004` (1,624) · `2018` (1,435) · `2016` (1,103) · `2003` (1,004) · `2000` (911) · `2013` (792) · `2015` (712) · `2017` (622) · `2014` (533) · `2001` (498) · `2011` (456) · `2021` (277) · `2023` (260) · `2020` (216) · `2024` (208) · `2012` (199) · `2022` (136) · `1999` (90) · `1998` (75) · `2025` (65) · `2008` (43) · `2010` (28) · `2009` (22) · `1900` (5) |
| `erstellung_monat` | MEDIUMINT | 9,604 | 10 | `7` (6,899) · `6` (6,529) · `4` (400) · `5` (112) · `3` (80) · `11` (40) · `8` (37) · `9` (26) · `10` (17) · `1` (5) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (22,036) · `Uebertragen` (1,608) · `Real` (104) · `Restrukturiert` (1) |
| `herkunft` | TEXT(50) | 0 | 3 | `swisstopo` (15,027) · `GIS Landesgrenze` (6,871) · `V25` (1,851) |
| `herkunft_jahr` | MEDIUMINT | 0 | 28 | `2019` (7,246) · `2018` (2,994) · `2016` (2,763) · `2014` (1,752) · `2017` (1,642) · `2015` (1,580) · `2013` (1,501) · `2021` (580) · `2006` (529) · `2000` (445) · `2023` (397) · `2002` (347) · `2024` (329) · `2020` (293) · `2022` (248) · `2012` (233) · `2005` (219) · `2011` (204) · `2004` (168) · `2025` (98) · `2003` (88) · `2001` (50) · `2008` (17) · `1998` (7) · `2009` (7) · `1900` (5) · `1999` (4) · `2010` (3) |
| `herkunft_monat` | MEDIUMINT | 1,857 | 11 | `6` (14,496) · `7` (6,904) · `4` (158) · `3` (135) · `5` (101) · `11` (40) · `10` (21) · `8` (15) · `9` (14) · `1` (5) · `2` (3) |
| `objektart` | TEXT(50) | 0 | 10 | `Wasserversorgung` (7,393) · `Landesgrenzstein` (6,885) · `Bildstock` (5,080) · `Wasserfall` (2,790) · `Gipfelkreuz` (446) · `Quelle` (329) · `Brunnen` (272) · `Denkmal` (249) · `Grotte, Hoehle` (247) · `Triangulationspyramide` (58) |
| `revision_jahr` | MEDIUMINT | 26 | 7 | `2024` (7,909) · `2023` (7,485) · `2019` (3,105) · `2025` (2,886) · `2022` (2,269) · `2020` (65) · `2021` (4) |
| `revision_monat` | MEDIUMINT | 26 | 1 | `6` (23,723) |
| `revision_qualitaet` | TEXT(50) | 129 | 3 | `Akt` (20,443) · `2020_Akt` (3,176) · `2017_Aufbau` (1) |
| `name` | TEXT(100) | 16,298 | >300 | _high cardinality — not enumerated_ |
| `tlm_eo_name_uuid` | TEXT(100) | 16,298 | >300 | _high cardinality — not enumerated_ |

### `tlm_bauten_mauer`

*tlm_bauten_mauer*

**16,675** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 3 | 26 | `2015` (5,478) · `2016` (2,459) · `2018` (2,160) · `2017` (1,235) · `2014` (889) · `2021` (757) · `2019` (691) · `2006` (626) · `2023` (460) · `2020` (348) · `2024` (335) · `2022` (318) · `2003` (205) · `2012` (192) · `2004` (182) · `2013` (123) · `2005` (84) · `2001` (45) · `2002` (26) · `2025` (22) · `2000` (12) · `2009` (10) · `1998` (5) · `2010` (4) · `2011` (3) · `1999` (3) |
| `erstellung_monat` | MEDIUMINT | 1,191 | 7 | `6` (15,282) · `3` (150) · `5` (19) · `7` (17) · `8` (7) · `9` (5) · `4` (4) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (15,420) · `Uebertragen` (682) · `Real` (527) · `Restrukturiert` (46) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (15,993) · `V25` (682) |
| `herkunft_jahr` | MEDIUMINT | 0 | 25 | `2015` (4,881) · `2018` (2,471) · `2016` (2,352) · `2021` (1,143) · `2017` (1,057) · `2019` (782) · `2014` (779) · `2023` (746) · `2022` (624) · `2006` (422) · `2020` (421) · `2024` (418) · `2012` (176) · `2013` (106) · `2004` (90) · `2003` (64) · `2001` (36) · `2005` (36) · `2025` (24) · `2002` (19) · `2000` (10) · `2009` (7) · `1998` (5) · `2010` (3) · `2011` (3) |
| `herkunft_monat` | MEDIUMINT | 682 | 7 | `6` (15,811) · `3` (140) · `5` (22) · `7` (9) · `8` (7) · `4` (2) · `9` (2) |
| `objektart` | TEXT(50) | 0 | 1 | `Mauer` (16,675) |
| `revision_jahr` | MEDIUMINT | 0 | 6 | `2024` (8,209) · `2023` (4,331) · `2022` (2,851) · `2025` (724) · `2019` (555) · `2020` (5) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (16,675) |
| `revision_qualitaet` | TEXT(50) | 52 | 2 | `Akt` (16,064) · `2020_Akt` (559) |

### `tlm_bauten_sportbaute_ply`

*tlm_bauten_sportbaute_ply*

**16,641** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 18 | `2010` (3,150) · `2011` (3,004) · `2012` (2,694) · `2013` (1,967) · `2016` (1,116) · `2018` (882) · `2017` (606) · `2014` (571) · `2015` (516) · `2019` (430) · `2020` (410) · `2021` (355) · `2024` (307) · `2023` (218) · `2022` (197) · `2008` (159) · `2025` (44) · `2009` (15) |
| `erstellung_monat` | MEDIUMINT | 0 | 7 | `6` (11,922) · `7` (1,626) · `5` (1,560) · `3` (1,042) · `8` (453) · `4` (35) · `9` (3) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (15,529) · `Real` (981) · `Restrukturiert` (130) · `Uebertragen` (1) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (16,641) |
| `herkunft_jahr` | MEDIUMINT | 0 | 18 | `2016` (2,371) · `2010` (1,853) · `2012` (1,761) · `2011` (1,739) · `2013` (1,418) · `2018` (1,257) · `2017` (1,133) · `2015` (1,012) · `2014` (999) · `2019` (758) · `2020` (559) · `2024` (550) · `2021` (473) · `2022` (322) · `2023` (321) · `2025` (61) · `2008` (47) · `2009` (7) |
| `herkunft_monat` | MEDIUMINT | 0 | 7 | `6` (13,787) · `7` (1,169) · `5` (796) · `3` (663) · `8` (211) · `4` (14) · `9` (1) |
| `objektart` | TEXT(50) | 0 | 1 | `Sportplatz` (16,641) |
| `revision_jahr` | MEDIUMINT | 0 | 8 | `2024` (6,433) · `2023` (4,366) · `2019` (3,759) · `2025` (1,082) · `2022` (828) · `2020` (169) · `2021` (2) · `2014` (2) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (16,641) |
| `revision_qualitaet` | TEXT(50) | 151 | 3 | `Akt` (12,476) · `2020_Akt` (4,013) · `2018_Aufbau` (1) |
| `tlm_bauten_name_uuid` | TEXT(38) | 16,639 | 2 | `{2C999E80-CC22-46BA-87D7-DDC1F15C5FAE}` (1) · `{A6BB78F3-272C-40B8-A5E0-C7C4F3F3160C}` (1) |
| `name` | TEXT(254) | 16,639 | 2 | `Kunsteisbahn Dolder` (1) · `Ottmar Hitzfeld GsponArena` (1) |

### `tlm_name_gelaendename`

*tlm_name_gelaendename*

**15,966** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 17 | `2009` (4,413) · `2019` (2,137) · `2018` (1,771) · `2016` (1,713) · `2008` (1,191) · `2014` (887) · `2015` (835) · `2017` (809) · `2013` (534) · `2011` (432) · `2012` (353) · `2010` (305) · `2024` (169) · `2023` (157) · `2020` (101) · `2022` (85) · `2025` (74) |
| `erstellung_monat` | MEDIUMINT | 5,120 | 12 | `6` (9,676) · `5` (248) · `11` (183) · `10` (122) · `12` (122) · `1` (118) · `8` (117) · `4` (115) · `3` (80) · `7` (54) · `2` (6) · `9` (5) |
| `grund_aenderung` | TEXT(50) | 0 | 3 | `Verbessert` (15,854) · `Real` (105) · `Restrukturiert` (7) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (15,966) |
| `herkunft_jahr` | MEDIUMINT | 0 | 14 | `2023` (10,964) · `2025` (2,471) · `2024` (1,683) · `2019` (380) · `2018` (160) · `2016` (126) · `2017` (69) · `2020` (36) · `2014` (22) · `2011` (16) · `2022` (14) · `2015` (11) · `2012` (10) · `2013` (4) |
| `herkunft_monat` | MEDIUMINT | 0 | 8 | `6` (15,935) · `12` (11) · `1` (5) · `10` (5) · `3` (3) · `8` (3) · `2` (2) · `11` (2) |
| `objektart` | TEXT(50) | 0 | 8 | `Graben` (8,852) · `Tal` (3,616) · `Grat` (2,527) · `Gletscher` (754) · `Massiv` (67) · `Haupttal` (65) · `Seeteil` (47) · `Huegelzug` (38) |
| `revision_jahr` | MEDIUMINT | 0 | 1 | `2025` (15,966) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (15,966) |
| `revision_qualitaet` | TEXT(50) | 0 | 1 | `TGMG_2020_Akt` (15,966) |
| `tlm_namen_name_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 0 | >300 | _high cardinality — not enumerated_ |

### `tlm_areale_verkehrsareal`

*tlm_areale_verkehrsareal*

**15,167** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 27 | `2010` (3,631) · `2012` (1,090) · `2011` (1,087) · `2008` (1,060) · `2009` (916) · `2015` (892) · `2021` (826) · `2016` (763) · `2019` (746) · `2014` (713) · `2013` (615) · `2018` (601) · `2020` (521) · `2022` (520) · `2023` (474) · `2024` (398) · `2017` (207) · `2025` (65) · `2004` (13) · `2007` (7) · `2002` (5) · `2006` (5) · `2000` (4) · `2005` (3) · `1998` (3) · `2001` (1) · `1900` (1) |
| `erstellung_monat` | MEDIUMINT | 34 | 10 | `6` (8,610) · `4` (2,066) · `3` (1,703) · `7` (1,241) · `5` (907) · `8` (456) · `9` (145) · `10` (3) · `1` (1) · `2` (1) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (13,690) · `Real` (1,133) · `Restrukturiert` (343) · `Uebertragen` (1) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (15,166) · `SchweizMobil` (1) |
| `herkunft_jahr` | MEDIUMINT | 0 | 19 | `2021` (2,019) · `2019` (2,011) · `2020` (1,742) · `2024` (1,535) · `2023` (1,208) · `2022` (1,123) · `2018` (986) · `2016` (856) · `2010` (795) · `2015` (604) · `2014` (522) · `2013` (336) · `2017` (331) · `2012` (305) · `2011` (268) · `2025` (205) · `2008` (180) · `2009` (137) · `2007` (4) |
| `herkunft_monat` | MEDIUMINT | 0 | 11 | `6` (13,701) · `4` (448) · `3` (446) · `7` (268) · `5` (197) · `8` (88) · `9` (12) · `2` (3) · `10` (2) · `1` (1) · `11` (1) |
| `name` | TEXT(254) | 13,971 | >300 | _high cardinality — not enumerated_ |
| `objektart` | TEXT(50) | 0 | 10 | `Oeffentliches Parkplatzareal` (10,107) · `Privates Parkplatzareal` (4,270) · `Verkehrsflaeche` (437) · `Rastplatzareal` (168) · `Privates Fahrareal` (101) · `Flugfeldareal` (39) · `Heliport` (21) · `Flugplatzareal` (17) · `Gleisareal` (5) · `Flughafenareal` (2) |
| `revision_jahr` | MEDIUMINT | 0 | 7 | `2024` (5,710) · `2023` (4,638) · `2019` (2,333) · `2022` (1,269) · `2025` (1,126) · `2020` (67) · `2021` (24) |
| `revision_monat` | MEDIUMINT | 0 | 3 | `6` (15,165) · `8` (1) · `10` (1) |
| `revision_qualitaet` | TEXT(50) | 319 | 2 | `Akt` (12,129) · `2020_Akt` (2,719) |
| `tlm_areale_name_uuid` | TEXT(38) | 13,971 | >300 | _high cardinality — not enumerated_ |

### `tlm_namen_name_pkt`

*tlm_namen_name_pkt*

**12,529** rows · geometry `geom` (POINT, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POINT | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 17 | `2009` (6,807) · `2008` (2,117) · `2013` (521) · `2019` (513) · `2010` (416) · `2018` (386) · `2016` (384) · `2012` (322) · `2014` (290) · `2015` (245) · `2011` (179) · `2017` (153) · `2023` (56) · `2024` (39) · `2025` (38) · `2022` (33) · `2020` (30) |
| `erstellung_monat` | MEDIUMINT | 8,171 | 11 | `6` (2,575) · `8` (389) · `3` (342) · `2` (276) · `12` (273) · `9` (155) · `4` (125) · `5` (100) · `11` (66) · `10` (56) · `7` (1) |
| `grund_aenderung` | TEXT(50) | 0 | 2 | `Verbessert` (12,489) · `Real` (40) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (12,529) |
| `herkunft_jahr` | MEDIUMINT | 0 | 15 | `2016` (2,370) · `2013` (1,814) · `2012` (1,674) · `2014` (1,588) · `2015` (1,357) · `2017` (949) · `2018` (849) · `2019` (845) · `2024` (352) · `2025` (242) · `2023` (195) · `2020` (129) · `2022` (122) · `2011` (40) · `2008` (3) |
| `herkunft_monat` | MEDIUMINT | 1 | 8 | `6` (10,336) · `2` (1,652) · `12` (470) · `3` (57) · `5` (9) · `11` (2) · `8` (1) · `10` (1) |
| `objektart` | TEXT(50) | 0 | 8 | `Huegel` (2,825) · `Hauptgipfel` (2,774) · `Gipfel` (2,696) · `Pass` (2,309) · `Haupthuegel` (1,347) · `Felskopf` (251) · `Alpiner Gipfel` (203) · `Strassenpass` (124) |
| `revision_jahr` | MEDIUMINT | 0 | 8 | `2025` (12,509) · `2024` (8) · `2017` (4) · `2016` (3) · `2023` (2) · `2015` (1) · `2020` (1) · `2019` (1) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (12,529) |
| `revision_qualitaet` | TEXT(50) | 9 | 4 | `TGMG_2020_Akt` (12,509) · `TLM_GN_Leer` (7) · `2015_Aufbau` (3) · `2018_Aufbau` (1) |
| `hoehe` | MEDIUMINT | 0 | >300 | _high cardinality — not enumerated_ |
| `tlm_namen_name_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 0 | >300 | _high cardinality — not enumerated_ |

### `tlm_bauten_versorgungsbaute_pkt`

*tlm_bauten_versorgungsbaute_pkt*

**6,831** rows · geometry `geom` (POINT, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POINT | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 28 | `2016` (1,265) · `2018` (717) · `2017` (521) · `2015` (520) · `2014` (517) · `2019` (475) · `2020` (392) · `2021` (331) · `2024` (280) · `2022` (264) · `2013` (222) · `2004` (216) · `2010` (181) · `2023` (155) · `2005` (147) · `2012` (147) · `2003` (128) · `2002` (110) · `2006` (68) · `2008` (56) · `2025` (49) · `2011` (24) · `2001` (18) · `1999` (12) · `2000` (8) · `1998` (4) · `2009` (2) · `1900` (2) |
| `erstellung_monat` | MEDIUMINT | 711 | 8 | `6` (5,799) · `3` (115) · `5` (87) · `7` (73) · `4` (37) · `8` (6) · `1` (2) · `9` (1) |
| `grund_aenderung` | TEXT(50) | 0 | 2 | `Verbessert` (6,267) · `Real` (564) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (6,831) |
| `herkunft_jahr` | MEDIUMINT | 0 | 18 | `2016` (962) · `2018` (904) · `2019` (751) · `2024` (681) · `2021` (633) · `2020` (526) · `2022` (516) · `2017` (490) · `2014` (399) · `2023` (346) · `2015` (341) · `2025` (86) · `2013` (77) · `2010` (58) · `2012` (46) · `2008` (13) · `2011` (1) · `1900` (1) |
| `herkunft_monat` | MEDIUMINT | 0 | 7 | `6` (6,737) · `3` (36) · `7` (27) · `5` (24) · `4` (5) · `1` (1) · `9` (1) |
| `objektart` | TEXT(50) | 0 | 3 | `Antenne klein` (6,544) · `Antenne gross` (202) · `Windturbine` (85) |
| `revision_jahr` | MEDIUMINT | 0 | 7 | `2024` (2,693) · `2023` (1,837) · `2019` (1,059) · `2022` (606) · `2025` (566) · `2020` (67) · `2021` (3) |
| `revision_monat` | MEDIUMINT | 0 | 2 | `6` (6,830) · `9` (1) |
| `revision_qualitaet` | TEXT(50) | 142 | 2 | `Akt` (5,419) · `2020_Akt` (1,270) |

### `tlm_namen_gebietsname`

*tlm_namen_gebietsname*

**3,927** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | 55 | `2023-12-15` (649) · `2023-12-14` (647) · `2023-12-13` (494) · `2025-12-15` (487) · `2023-12-08` (307) · `2023-12-07` (208) · `2024-12-16` (206) · `2023-12-12` (181) · `2025-12-12` (171) · `2025-12-11` (171) · `2023-12-11` (72) · `2024-12-18` (67) · `2025-10-29` (48) · `2024-12-11` (42) · `2024-12-13` (39) · `2024-12-10` (26) · `2024-12-09` (26) · `2024-12-12` (19) · `2024-12-17` (7) · `2025-10-30` (6) · `2023-12-19` (6) · `2023-12-18` (6) · `2025-12-08` (4) · `2024-03-13` (3) · `2024-02-05` (3) · `2024-11-13` (2) · `2024-10-22` (2) · `2025-08-25` (1) · `2024-09-25` (1) · `2024-01-29` (1) · `2025-09-16` (1) · `2024-09-04` (1) · `2024-01-25` (1) · `2025-09-15` (1) · `2014-12-11` (1) · `2025-07-15` (1) · `2024-07-24` (1) · `2024-02-09` (1) · `2024-02-14` (1) · `2024-02-20` (1) · … +15 more |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 17 | `2016` (728) · `2013` (475) · `2018` (426) · `2015` (418) · `2014` (344) · `2012` (311) · `2019` (286) · `2017` (260) · `2011` (188) · `2009` (182) · `2023` (81) · `2024` (74) · `2020` (45) · `2010` (40) · `2025` (39) · `2022` (25) · `2008` (5) |
| `erstellung_monat` | MEDIUMINT | 46 | 11 | `6` (3,325) · `5` (197) · `11` (85) · `1` (62) · `3` (58) · `8` (38) · `10` (37) · `4` (37) · `12` (24) · `9` (11) · `2` (7) |
| `grund_aenderung` | TEXT(50) | 0 | 3 | `Verbessert` (3,853) · `Real` (73) · `Restrukturiert` (1) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (3,927) |
| `herkunft_jahr` | MEDIUMINT | 0 | 7 | `2023` (2,570) · `2025` (893) · `2024` (459) · `2018` (2) · `2013` (1) · `2016` (1) · `2019` (1) |
| `herkunft_monat` | MEDIUMINT | 0 | 1 | `6` (3,927) |
| `objektart` | TEXT(50) | 0 | 3 | `Gebiet` (3,839) · `Landschaftsname` (77) · `Grossregion` (11) |
| `revision_jahr` | MEDIUMINT | 0 | 1 | `2025` (3,927) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (3,927) |
| `revision_qualitaet` | TEXT(50) | 0 | 1 | `TGMG_2020_Akt` (3,927) |
| `tlm_namen_name_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 0 | >300 | _high cardinality — not enumerated_ |

### `tlm_areale_freizeitareal`

*tlm_areale_freizeitareal*

**3,400** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 17 | `2010` (733) · `2013` (557) · `2012` (548) · `2011` (529) · `2008` (242) · `2009` (120) · `2016` (109) · `2015` (99) · `2014` (88) · `2021` (60) · `2019` (54) · `2023` (53) · `2020` (48) · `2024` (48) · `2018` (43) · `2022` (38) · `2017` (31) |
| `erstellung_monat` | MEDIUMINT | 0 | 7 | `6` (1,950) · `3` (391) · `5` (367) · `7` (304) · `4` (224) · `8` (134) · `9` (30) |
| `grund_aenderung` | TEXT(50) | 0 | 3 | `Verbessert` (2,999) · `Real` (226) · `Restrukturiert` (175) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (3,400) |
| `herkunft_jahr` | MEDIUMINT | 0 | 17 | `2021` (494) · `2022` (458) · `2019` (377) · `2023` (305) · `2016` (304) · `2020` (294) · `2024` (287) · `2018` (224) · `2013` (148) · `2015` (120) · `2014` (115) · `2017` (86) · `2010` (86) · `2012` (66) · `2011` (20) · `2025` (15) · `2008` (1) |
| `herkunft_monat` | MEDIUMINT | 0 | 6 | `6` (3,295) · `3` (39) · `7` (39) · `5` (23) · `8` (3) · `4` (1) |
| `name` | TEXT(254) | 1,189 | >300 | _high cardinality — not enumerated_ |
| `objektart` | TEXT(50) | 0 | 8 | `Sportplatzareal` (1,209) · `Schwimmbadareal` (854) · `Standplatzareal` (457) · `Campingplatzareal` (441) · `Golfplatzareal` (198) · `Freizeitanlagenareal` (143) · `Zooareal` (89) · `Pferderennbahnareal` (9) |
| `revision_jahr` | MEDIUMINT | 0 | 6 | `2024` (1,264) · `2023` (951) · `2022` (481) · `2019` (451) · `2025` (229) · `2020` (24) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (3,400) |
| `revision_qualitaet` | TEXT(50) | 24 | 2 | `Akt` (2,634) · `2020_Akt` (742) |
| `tlm_areale_name_uuid` | TEXT(38) | 1,189 | >300 | _high cardinality — not enumerated_ |

### `tlm_bauten_sportbaute_lin`

*tlm_bauten_sportbaute_lin*

**3,160** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 18 | `2013` (659) · `2011` (529) · `2010` (508) · `2012` (471) · `2008` (337) · `2014` (173) · `2009` (106) · `2016` (70) · `2022` (68) · `2021` (57) · `2015` (53) · `2018` (31) · `2017` (26) · `2019` (26) · `2023` (20) · `2024` (14) · `2020` (11) · `2025` (1) |
| `erstellung_monat` | MEDIUMINT | 0 | 7 | `6` (1,363) · `3` (741) · `4` (482) · `5` (317) · `7` (125) · `8` (76) · `9` (56) |
| `grund_aenderung` | TEXT(50) | 0 | 3 | `Verbessert` (3,093) · `Real` (58) · `Restrukturiert` (9) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (3,160) |
| `herkunft_jahr` | MEDIUMINT | 0 | 18 | `2021` (422) · `2022` (379) · `2013` (309) · `2016` (284) · `2014` (283) · `2018` (270) · `2019` (215) · `2012` (194) · `2023` (175) · `2024` (142) · `2017` (137) · `2020` (89) · `2011` (84) · `2015` (82) · `2010` (80) · `2025` (8) · `2009` (4) · `2008` (3) |
| `herkunft_monat` | MEDIUMINT | 0 | 7 | `6` (2,861) · `3` (176) · `5` (59) · `7` (45) · `4` (11) · `8` (6) · `9` (2) |
| `objektart` | TEXT(50) | 0 | 6 | `Scheibenstand` (2,883) · `Laufbahn` (197) · `Pferderennbahn` (30) · `Rodelbahn` (25) · `Skisprungschanze` (24) · `Bobbahn` (1) |
| `revision_jahr` | MEDIUMINT | 0 | 6 | `2024` (1,294) · `2023` (665) · `2022` (466) · `2019` (398) · `2025` (318) · `2020` (19) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (3,160) |
| `revision_qualitaet` | TEXT(50) | 13 | 2 | `Akt` (2,525) · `2020_Akt` (622) |
| `tlm_bauten_name_uuid` | TEXT(38) | 3,110 | 47 | `{1068172B-5204-4E54-AD5F-C577539D869D}` (2) · `{73316536-C6DE-43F4-8568-A5B5E766DBE1}` (2) · `{5C98B974-A60E-4DA9-AD19-6DF36617CC2B}` (2) · `{89A9F895-3F87-44E5-9AC8-DD46E9F43ED7}` (1) · `{7145E5A6-16CE-4727-BDF1-335454459F58}` (1) · `{CF8E106D-2FC7-4CA1-8814-397BF057E162}` (1) · `{2D623C22-FDCD-4800-BFEC-2EDC03C2D5ED}` (1) · `{3DCF0331-876A-4E7C-BEA0-49C62CD2EAF7}` (1) · `{65AAD4E9-139D-4D86-989A-9A1D38EB5CC5}` (1) · `{BAC75E27-48D0-4B0F-81FF-222021ADA439}` (1) · `{B75B4163-D56F-42F5-8F02-1A00F8CA80DA}` (1) · `{DA2DAF39-1A12-4C7C-914F-9B5B5AB93B3C}` (1) · `{BDC099C5-F2E6-4C4C-8F50-1C5927D20320}` (1) · `{00813D4F-59B8-44B7-B2B1-A2252F44C7FC}` (1) · `{90FD7B79-7410-43C3-986E-C368D2848045}` (1) · `{BB8BDD58-14F6-4443-940E-9E026A939F83}` (1) · `{6B8EFFF5-BDF0-4B7C-B2D8-59F6098D1E6B}` (1) · `{8DB8C276-3984-4D4A-AE50-553336F5E308}` (1) · `{E8C77DCD-4C4E-4082-9552-04A09665A12E}` (1) · `{2A99137E-8A86-4F6B-A82A-26279377BAE5}` (1) · `{DE63A888-EE68-456D-93FA-2539650951AC}` (1) · `{F29097CC-50B4-4A42-98B2-B14EE2C245F7}` (1) · `{401AED1F-0CEA-46CD-B38E-D2C9A8B2D283}` (1) · `{B9042F46-8947-427A-9BD0-0616AA94BEC5}` (1) · `{0157B740-AAB0-4762-B3A6-277935F295B5}` (1) · `{09E16302-31FD-4B0D-BB4A-C42FC915667D}` (1) · `{66EA3CA5-22E9-42CE-9718-F42C28B41ABA}` (1) · `{4AF94FEC-9763-40EB-B2ED-D7AEA6B9A071}` (1) · `{63350861-4E5A-4ED2-A80B-73E9E771ECEF}` (1) · `{AF0AEA4E-2857-4617-8507-F812D26F5FDF}` (1) · `{4368F835-C5CC-4477-8305-1FD7AEF3F4FC}` (1) · `{6E0B6832-025F-4DAD-96A5-1EA896C5F9B8}` (1) · `{15903928-2451-4F87-A060-3E2C1A4DFF1C}` (1) · `{886292ED-A369-49D5-A6C1-520EB96EA821}` (1) · `{830A2E76-48D6-47F6-8A15-C13C47B08767}` (1) · `{523F5560-59EE-4084-91EA-1A949831EEB0}` (1) · `{65C38836-0D85-485A-8A13-53CE67AB389F}` (1) · `{DDB30E05-81A2-4382-9EA7-BDC8D1D1C65D}` (1) · `{EBE30C09-17E1-4F97-A6A0-97BF7023DFC5}` (1) · `{341DF0A5-7010-4FA9-B156-6E940D04CE89}` (1) · … +7 more |
| `name` | TEXT(254) | 3,110 | 47 | `Schrattenschanzen` (2) · `Wasserschanzen Trainingscenter Jumpin` (2) · `Tremplin Big Air Leysin` (2) · `Kronbergbobbahn` (1) · `Schanzen Kollersweid, Walter Steiner Schanze` (1) · `Alpe Foppa` (1) · `Sommer-Schlittelbahn Schatzalp` (1) · `Rodelbahn Pradaschier` (1) · `Olympia Bob Run St.Moritz-Celerina` (1) · `Sommer-Rodelbahn Schwarzsee` (1) · `Bob-Luge de Moléson` (1) · `Rodelbahn Oeschinensee` (1) · `Toboggan à la Vue-des-Alpes` (1) · `Solarbob Langenbruck` (1) · `Sommerrodelbahn Rischli` (1) · `Sommerrodelbahn Ristis` (1) · `Sommerrodelbahn Stuckli Run` (1) · `Rodelbahn Heimwehfluh` (1) · `Bachtelblick-Schanze` (1) · `Grosse Panoramaschanze` (1) · `Kleine Panoramaschanze` (1) · `Atzmännig` (1) · `BobBahn Schongi-Land` (1) · `Sommer-Rodelbahn Fräkmüntegg` (1) · `Schanzen Einsiedeln, Andreas Küttel-Schanze` (1) · `Schanzen Einsiedeln, Simon Ammann-Schanze` (1) · `Schanzen Einsiedeln, Grosse KPT-Schanze` (1) · `Schanzen Einsiedeln, Kleine KPT-Schanze` (1) · `Sommer-Rodelbahn Wirzweli` (1) · `Floomzer` (1) · `Schanzen Kollersweid, Kleine Schanze` (1) · `Schanzen Kollersweid, Bubenschanze` (1) · `Rodelbahn Pfingstegg` (1) · `Nordic Arena Kandersteg, Bire-Schanze` (1) · `Nordic Arena Kandersteg, Lötschberg-Schanze` (1) · `Nordic Arena Kandersteg, Blümlisalp-Schanze` (1) · `Gurten` (1) · `Tremplin en la Tré` (1) · `Luge Féeline` (1) · `Gross-Titlis-Schanze` (1) · … +7 more |

### `tlm_bauten_verkehrsbaute_ply`

*tlm_bauten_verkehrsbaute_ply*

**2,965** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 27 | `2010` (985) · `2009` (644) · `2008` (272) · `2011` (246) · `2019` (128) · `2014` (116) · `2018` (92) · `2020` (79) · `2013` (72) · `2016` (71) · `2021` (44) · `2015` (28) · `2024` (27) · `2022` (24) · `2023` (24) · `2012` (20) · `2004` (18) · `2017` (18) · `2002` (12) · `2000` (11) · `2025` (10) · `2005` (7) · `2006` (7) · `2007` (5) · `1998` (3) · `2003` (1) · `2001` (1) |
| `erstellung_monat` | MEDIUMINT | 60 | 7 | `6` (1,590) · `4` (349) · `3` (328) · `7` (327) · `8` (159) · `9` (82) · `5` (70) |
| `grund_aenderung` | TEXT(50) | 0 | 4 | `Verbessert` (2,327) · `Real` (453) · `Restrukturiert` (163) · `Uebertragen` (22) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (2,940) · `Transportunternehmen` (25) |
| `herkunft_jahr` | MEDIUMINT | 0 | 18 | `2019` (512) · `2018` (482) · `2020` (439) · `2021` (297) · `2024` (287) · `2023` (186) · `2010` (180) · `2022` (122) · `2025` (109) · `2016` (101) · `2009` (63) · `2013` (39) · `2011` (37) · `2008` (35) · `2017` (35) · `2014` (20) · `2012` (11) · `2015` (10) |
| `herkunft_monat` | MEDIUMINT | 0 | 9 | `6` (2,783) · `4` (56) · `7` (43) · `3` (40) · `8` (19) · `5` (16) · `9` (4) · `10` (2) · `11` (2) |
| `objektart` | TEXT(50) | 0 | 6 | `Perron` (2,793) · `Rollfeld Hartbelag` (75) · `Hartbelagpiste` (41) · `Graspiste` (36) · `Rollfeld Gras` (15) · `Schleuse` (5) |
| `revision_jahr` | MEDIUMINT | 0 | 6 | `2024` (1,164) · `2023` (829) · `2019` (475) · `2025` (283) · `2022` (208) · `2020` (6) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (2,965) |
| `revision_qualitaet` | TEXT(50) | 56 | 4 | `Akt` (2,337) · `2020_Akt` (565) · `2021_Akt` (4) · `2018_Aufbau` (3) |

### `tlm_oev_uebrige_bahn`

*tlm_oev_uebrige_bahn*

**2,903** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 1 | 28 | `2004` (391) · `2003` (375) · `2021` (263) · `2006` (251) · `2018` (232) · `2005` (175) · `2002` (159) · `2001` (108) · `2013` (106) · `1999` (104) · `2024` (101) · `2022` (71) · `2010` (67) · `2016` (55) · `1998` (51) · `2023` (50) · `2019` (48) · `2025` (44) · `2009` (35) · `2012` (31) · `2000` (30) · `2015` (30) · `2008` (28) · `2017` (27) · `2020` (27) · `2014` (25) · `2011` (16) · `1900` (2) |
| `erstellung_monat` | MEDIUMINT | 1,645 | 10 | `6` (1,098) · `8` (43) · `4` (30) · `3` (28) · `5` (25) · `9` (21) · `7` (9) · `1` (2) · `12` (1) · `10` (1) |
| `grund_aenderung` | TEXT(50) | 0 | 3 | `Verbessert` (2,685) · `Real` (205) · `Restrukturiert` (13) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (2,903) |
| `herkunft_jahr` | MEDIUMINT | 0 | 19 | `2021` (402) · `2023` (364) · `2024` (355) · `2018` (300) · `2010` (276) · `2022` (182) · `2013` (163) · `2019` (146) · `2008` (135) · `2009` (107) · `2016` (91) · `2025` (90) · `2020` (77) · `2014` (67) · `2015` (49) · `2017` (48) · `2012` (36) · `2011` (14) · `1900` (1) |
| `herkunft_monat` | MEDIUMINT | 0 | 10 | `6` (2,462) · `5` (143) · `4` (109) · `8` (87) · `9` (59) · `7` (24) · `3` (14) · `10` (3) · `2` (1) · `1` (1) |
| `objektart` | TEXT(50) | 0 | 7 | `Transportseil` (1,076) · `Skilift` (821) · `Sesselbahn` (366) · `Luftseilbahn` (322) · `Foerderband` (149) · `Gondelbahn` (144) · `Lift` (25) |
| `revision_jahr` | MEDIUMINT | 0 | 8 | `2024` (1,132) · `2023` (698) · `2022` (431) · `2025` (365) · `2019` (265) · `2020` (6) · `2021` (5) · `2015` (1) |
| `revision_monat` | MEDIUMINT | 0 | 2 | `6` (2,902) · `8` (1) |
| `revision_qualitaet` | TEXT(50) | 79 | 2 | `Akt` (2,526) · `2020_Akt` (298) |
| `tlm_oev_name_uuid` | TEXT(38) | 1,265 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 1,265 | >300 | _high cardinality — not enumerated_ |
| `ausser_betrieb` | TEXT(10) | 0 | 3 | `Falsch` (2,830) · `Wahr` (69) · `k_W` (4) |
| `stufe` | TEXT(5) | 0 | 7 | `1` (2,115) · `2` (659) · `0` (71) · `3` (53) · `4` (3) · `5` (1) · `-1` (1) |
| `betriebsbahn` | TEXT(10) | 0 | 3 | `Falsch` (1,608) · `Wahr` (1,291) · `k_W` (4) |
| `eroeffnungsdatum` | DATE | 2,902 | 1 | `2026-12-31` (1) |

### `tlm_bauten_leitung`

*tlm_bauten_leitung*

**2,882** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | 240 | `2021-10-29` (100) · `2021-10-12` (72) · `2021-10-27` (70) · `2021-10-15` (67) · `2025-12-02` (66) · `2021-09-15` (63) · `2021-11-29` (58) · `2021-10-13` (57) · `2021-12-03` (57) · `2021-10-20` (55) · `2021-10-14` (53) · `2021-10-18` (53) · `2021-11-02` (52) · `2021-10-26` (51) · `2021-10-19` (51) · `2021-11-03` (50) · `2021-09-29` (46) · `2022-04-01` (46) · `2021-10-28` (42) · `2021-11-26` (42) · `2021-12-01` (41) · `2021-09-28` (40) · `2021-11-30` (39) · `2021-09-14` (39) · `2021-11-19` (39) · `2025-01-21` (32) · `2021-10-22` (31) · `2021-09-13` (30) · `2021-12-02` (30) · `2021-12-06` (28) · `2021-09-17` (27) · `2021-12-09` (27) · `2022-04-22` (27) · `2024-03-21` (25) · `2021-10-11` (25) · `2021-09-08` (24) · `2023-09-28` (24) · `2021-11-18` (24) · `2021-07-01` (24) · `2021-07-19` (23) · … +200 more |
| `datum_erstellung` | DATE | 0 | 190 | `2009-01-08` (120) · `2021-10-12` (91) · `2021-10-27` (79) · `2021-09-28` (75) · `2021-09-15` (73) · `2021-10-15` (73) · `2021-10-26` (70) · `2021-10-19` (67) · `2021-10-14` (67) · `2021-10-13` (67) · `2021-11-29` (67) · `2021-10-20` (65) · `2021-10-29` (61) · `2021-09-29` (58) · `2021-11-02` (56) · `2021-09-13` (54) · `2021-12-01` (54) · `2021-09-14` (52) · `2021-10-18` (50) · `2021-11-30` (49) · `2021-12-03` (46) · `2021-11-26` (42) · `2008-01-18` (40) · `2021-11-03` (40) · `2021-11-18` (37) · `2021-11-19` (37) · `2022-04-01` (36) · `2021-10-22` (34) · `2021-07-19` (33) · `2021-12-02` (33) · `2021-09-17` (31) · `2021-10-28` (31) · `2021-10-11` (30) · `2021-10-01` (29) · `2021-09-08` (28) · `2021-12-09` (28) · `2021-11-25` (27) · `2021-12-06` (27) · `2021-11-24` (26) · `2021-07-21` (26) · … +150 more |
| `erstellung_jahr` | MEDIUMINT | 0 | 22 | `2021` (959) · `2019` (874) · `2020` (460) · `2018` (217) · `2023` (87) · `2024` (54) · `2004` (36) · `2005` (34) · `2022` (32) · `2003` (27) · `2001` (21) · `2006` (20) · `2017` (18) · `2000` (16) · `2016` (9) · `2002` (7) · `2013` (3) · `2014` (2) · `1999` (2) · `2015` (2) · `2025` (1) · `2010` (1) |
| `erstellung_monat` | MEDIUMINT | 0 | 3 | `6` (2,718) · `1` (163) · `3` (1) |
| `grund_aenderung` | TEXT(50) | 0 | 3 | `Verbessert` (2,624) · `Restrukturiert` (246) · `Real` (12) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (2,881) · `BAZL` (1) |
| `herkunft_jahr` | MEDIUMINT | 0 | 8 | `2021` (955) · `2019` (861) · `2020` (503) · `2024` (197) · `2018` (175) · `2023` (120) · `2022` (61) · `2025` (10) |
| `herkunft_monat` | MEDIUMINT | 0 | 1 | `6` (2,882) |
| `objektart` | TEXT(50) | 0 | 1 | `Stromleitung` (2,882) |
| `revision_jahr` | MEDIUMINT | 0 | 8 | `2024` (1,277) · `2023` (601) · `2019` (601) · `2025` (230) · `2022` (151) · `2021` (16) · `2018` (5) · `2020` (1) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (2,882) |
| `revision_qualitaet` | TEXT(50) | 0 | 4 | `Akt` (2,845) · `2020_Akt` (26) · `TLM_2022_RG_Akt` (10) · `2021_Akt` (1) |
| `stufe` | TEXT(50) | 0 | 1 | `3` (2,882) |
| `netzebene` | TEXT(50) | 0 | 3 | `3` (2,039) · `1` (646) · `ub` (197) |
| `name` | TEXT(50) | 2,266 | 151 | `Airolo-Mettlen` (28) · `Bonaduz-Rüthi` (13) · `Breite-Tavanasa` (12) · `Lachmatt-Laufenburg` (12) · `Benken-Sils` (12) · `Laufenburg-Mühleberg` (12) · `Chamoson-Romanel` (11) · `Bassecourt-Mühleberg` (11) · `Airolo-Mörel` (11) · `Grynau-Samstagern` (10) · `Breite-Weinfelden` (10) · `Mühleberg-St-Triphon` (10) · `Gösgen-Lachmatt` (10) · `Bickigen-Innertkirchen` (10) · `Handeck-Peccia` (9) · `Grynau-Winkeln` (8) · `Romanel-St-TriphonRomanel-St-Triphon` (8) · `Benken-Samstagern` (8) · `Chamoson-Mühleberg` (8) · `Iragna-Magadino` (8) · `Sils-Tinzen` (8) · `Beznau-Niederwil` (8) · `Bickigen-Chippis` (8) · `Grimsel-Innertkirchen` (7) · `Oftringen-Rupperswil` (7) · `Pradella-Sils` (7) · `Lavorgo-Mettlen` (7) · `Y Fehraltorf-Aathal` (7) · `Innertkirchen-Mettlen` (7) · `Gösgen-Mettlen` (7) · `Galmiz-Mühleberg` (7) · `Bickigen-Flumenthal` (6) · `Gösgen-Mettlen (via Sursee)` (6) · `Bonaduz-Breite` (6) · `Lavorgo-Musignano (IT)` (6) · `Magadino-Manno` (6) · `Rüthi-Winkeln` (6) · `Foretaille-Romanel` (6) · `Y La Punt-Robbia` (6) · `Y Birr-Rupperswil` (6) · … +111 more |
| `tlm_bauten_name_uuid` | TEXT(50) | 2,266 | 159 | `{A03EC3B2-F8A0-43E6-ADE0-73C2EE632B38}` (24) · `{7D42C176-3759-4368-A3A4-C921754D5DB7}` (13) · `{4E9CE761-FEDA-44E4-9F1E-FDEC636EC57F}` (12) · `{07A2656D-F897-444E-8A50-9BDDFAF1A4B8}` (12) · `{678F3913-F777-428A-92C1-BF1FAA46A794}` (12) · `{16C79DE7-E591-40DC-804F-FBDD9A2CA306}` (11) · `{B4E5E431-F148-472C-A8C8-E5022093DFD0}` (11) · `{C9DCF6AD-8231-42F2-817A-89F3719F4461}` (11) · `{3460766C-C42F-4188-9311-872186A3E395}` (10) · `{7A04F048-2653-461A-B6CC-8111E39118F6}` (10) · `{94001C62-F377-4605-8700-082CB495C996}` (10) · `{5C72644C-0529-4DE2-8AEE-AB87971EBF9D}` (10) · `{350DADE0-1D25-4A5A-B7C4-62A8D8A179B5}` (9) · `{5CEB315E-8809-4464-A848-548058FFA403}` (8) · `{9AAE92F5-4802-49B5-A178-D5B029F3E299}` (8) · `{69E8774F-B5CB-4B40-B09B-B0934795C43E}` (8) · `{68433851-9B5A-419A-A333-76DEDA10F7B0}` (8) · `{388365E8-4429-4E00-A28C-41D7694CA08E}` (8) · `{EA73C1AA-EFAC-44CB-87C9-A24D3C700BDF}` (8) · `{DC88A798-B882-454D-97D2-CA571CEF8CBF}` (8) · `{FF71343D-627E-49DA-8385-BD37011FEC5F}` (7) · `{D568CC5F-E6BA-4FE0-A142-71B1DC1BDDF4}` (7) · `{0ED17AA4-7A5E-4655-9256-97AFC95B6984}` (7) · `{07659180-1BAF-4AA0-B529-4165BF8B0A98}` (7) · `{85FAA76B-E0D0-432F-81CB-A423AB190377}` (7) · `{45510F71-0D49-4823-AFB8-B9E22E6314AD}` (7) · `{BDD201CA-FDC0-4820-BD55-90E7041049E8}` (7) · `{6772FA9E-5550-401A-BAC9-A74504D5A98A}` (7) · `{DA073923-46D3-4E59-9315-B882DD5B82E6}` (6) · `{1012B835-3DF8-42DC-884B-B067EBD4CE0E}` (6) · `{70DE3C63-404C-469B-8F69-4F2BBD36B474}` (6) · `{C96BE305-79BE-4148-8B7C-04E1695BE3E9}` (6) · `{AACAF377-3976-492D-B05B-17108E959DF5}` (6) · `{9912B36B-95C8-42BC-AFAE-64CECD6D7AF0}` (6) · `{ED2AF2A2-727D-4A2C-8415-1B7DBD227D89}` (6) · `{8F281B5C-8A17-4730-81D2-5F0CC6EA3439}` (6) · `{22B13FC0-FF38-4C7A-8960-203163CABC19}` (6) · `{A75A1817-AEA4-43DB-B546-E32CF93C0C2D}` (6) · `{7E049B4B-909C-49B5-94FE-7E4CC2841E16}` (6) · `{745F1B2F-8E37-465E-B59B-8747FE441760}` (5) · … +119 more |

### `tlm_bauten_verkehrsbaute_lin`

*tlm_bauten_verkehrsbaute_lin*

**1,966** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | 272 | `2014-02-03` (162) · `2014-01-30` (106) · `2015-08-14` (106) · `2014-09-15` (75) · `2021-01-14` (63) · `2015-08-13` (58) · `2015-12-01` (51) · `2020-02-18` (47) · `2014-02-24` (40) · `2015-09-16` (37) · `2013-10-11` (36) · `2025-12-05` (36) · `2015-06-19` (30) · `2020-07-30` (28) · `2021-01-29` (27) · `2021-02-24` (26) · `2019-10-03` (26) · `2018-12-13` (26) · `2015-09-15` (25) · `2012-09-14` (23) · `2014-07-23` (21) · `2025-11-06` (18) · `2012-09-20` (16) · `2013-12-06` (16) · `2021-02-08` (16) · `2020-07-15` (16) · `2016-06-21` (15) · `2013-08-23` (14) · `2013-12-04` (14) · `2020-12-14` (14) · `2020-02-28` (13) · `2025-11-05` (12) · `2016-10-25` (12) · `2018-12-14` (12) · `2020-11-26` (12) · `2021-02-11` (12) · `2021-03-09` (11) · `2021-03-02` (11) · `2014-03-21` (11) · `2019-10-02` (10) · … +232 more |
| `datum_erstellung` | DATE | 0 | 197 | `2014-02-03` (179) · `2014-01-30` (142) · `2014-09-15` (139) · `2015-08-14` (127) · `2015-12-01` (88) · `2015-08-13` (82) · `2015-06-19` (73) · `2021-01-14` (59) · `2014-02-24` (43) · `2015-09-16` (42) · `2013-10-11` (38) · `2015-09-15` (32) · `2012-09-14` (28) · `2018-12-13` (28) · `2020-02-18` (28) · `2021-01-29` (27) · `2014-07-23` (26) · `2015-06-17` (25) · `2013-12-06` (22) · `2014-12-12` (22) · `2013-08-23` (21) · `2020-07-30` (20) · `2012-09-20` (18) · `2014-03-21` (18) · `2025-12-05` (16) · `2013-12-04` (15) · `2018-12-14` (15) · `2019-10-03` (15) · `2020-12-14` (15) · `2014-02-12` (13) · `2016-10-25` (13) · `2020-11-26` (13) · `2021-02-24` (13) · `2015-09-14` (12) · `2021-02-11` (12) · `2014-01-31` (11) · `2011-06-14` (10) · `2014-03-07` (10) · `2015-06-18` (10) · `2021-12-30` (10) · … +157 more |
| `erstellung_jahr` | MEDIUMINT | 0 | 19 | `2013` (648) · `2015` (308) · `2014` (227) · `2020` (182) · `2019` (120) · `2010` (95) · `2012` (83) · `2017` (53) · `2021` (53) · `2025` (43) · `2024` (35) · `2016` (30) · `2018` (29) · `2008` (16) · `2022` (15) · `2011` (14) · `2023` (12) · `2009` (2) · `1900` (1) |
| `erstellung_monat` | MEDIUMINT | 0 | 6 | `6` (1,830) · `3` (58) · `5` (35) · `7` (32) · `4` (10) · `1` (1) |
| `grund_aenderung` | TEXT(50) | 0 | 3 | `Verbessert` (1,863) · `Real` (90) · `Restrukturiert` (13) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (1,966) |
| `herkunft_jahr` | MEDIUMINT | 0 | 19 | `2013` (472) · `2020` (281) · `2015` (265) · `2019` (200) · `2014` (145) · `2021` (110) · `2025` (93) · `2010` (80) · `2017` (65) · `2018` (57) · `2024` (49) · `2016` (46) · `2012` (41) · `2023` (28) · `2022` (24) · `2011` (5) · `2008` (2) · `2009` (2) · `1900` (1) |
| `herkunft_monat` | MEDIUMINT | 0 | 6 | `6` (1,872) · `3` (40) · `7` (27) · `5` (20) · `4` (6) · `1` (1) |
| `objektart` | TEXT(50) | 0 | 1 | `Hafensteg` (1,966) |
| `revision_jahr` | MEDIUMINT | 0 | 8 | `2023` (557) · `2019` (554) · `2024` (536) · `2025` (193) · `2015` (90) · `2022` (28) · `2014` (4) · `2018` (4) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (1,966) |
| `revision_qualitaet` | TEXT(50) | 103 | 2 | `Akt` (1,296) · `2020_Akt` (567) |
| `stufe` | MEDIUMINT | 0 | 2 | `0` (1,957) · `1` (9) |

### `tlm_strassen_aus_einfahrt`

*tlm_strassen_aus_einfahrt*

**1,915** rows · geometry `geom` (POINT, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POINT | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | 289 | `2013-01-04` (132) · `2024-07-05` (47) · `2020-11-10` (36) · `2023-05-09` (36) · `2020-12-29` (34) · `2020-01-15` (32) · `2020-01-10` (29) · `2020-02-27` (29) · `2020-01-17` (26) · `2025-02-27` (25) · `2025-02-28` (25) · `2024-11-08` (25) · `2020-01-28` (25) · `2019-11-25` (23) · `2020-02-24` (23) · `2024-07-18` (23) · `2019-12-05` (22) · `2020-02-25` (22) · `2021-07-08` (22) · `2020-03-24` (21) · `2021-06-18` (21) · `2021-02-12` (21) · `2021-07-14` (20) · `2020-12-28` (20) · `2021-10-21` (20) · `2020-01-14` (20) · `2025-03-14` (19) · `2024-12-12` (19) · `2022-02-11` (19) · `2025-01-09` (18) · `2020-02-18` (18) · `2025-01-10` (17) · `2020-03-20` (16) · `2019-12-17` (15) · `2024-07-04` (15) · `2020-02-13` (15) · `2020-10-21` (14) · `2020-10-20` (14) · `2024-07-19` (14) · `2025-01-13` (14) · … +249 more |
| `datum_erstellung` | DATE | 0 | 286 | `2011-10-18` (66) · `2010-07-14` (44) · `2011-01-27` (40) · `2011-01-11` (38) · `2010-12-24` (37) · `2010-09-21` (31) · `2010-11-17` (31) · `2011-09-19` (31) · `2011-01-21` (29) · `2011-07-20` (29) · `2011-01-14` (28) · `2011-06-15` (28) · `2010-12-07` (25) · `2011-01-17` (25) · `2010-04-16` (24) · `2011-09-09` (24) · `2009-01-13` (23) · `2011-02-08` (23) · `2009-07-29` (22) · `2010-11-03` (22) · `2011-05-25` (22) · `2011-09-01` (22) · `2011-01-13` (20) · `2011-01-19` (20) · `2011-05-26` (20) · `2011-08-04` (20) · `2011-01-12` (19) · `2009-06-15` (18) · `2010-10-14` (18) · `2011-10-14` (18) · `2010-10-01` (17) · `2011-05-02` (17) · `2009-07-30` (16) · `2011-01-20` (16) · `2011-02-09` (16) · `2009-04-22` (15) · `2010-11-30` (15) · `2011-09-14` (15) · `2009-01-15` (14) · `2010-11-04` (14) · … +246 more |
| `erstellung_jahr` | MEDIUMINT | 0 | 19 | `2010` (687) · `2009` (566) · `2008` (305) · `2011` (84) · `2007` (69) · `2021` (48) · `2013` (22) · `2014` (21) · `2012` (19) · `2015` (16) · `2019` (16) · `2016` (13) · `2024` (12) · `2020` (11) · `2017` (8) · `2018` (6) · `2023` (6) · `2022` (5) · `2025` (1) |
| `erstellung_monat` | MEDIUMINT | 0 | 7 | `6` (573) · `7` (344) · `8` (328) · `5` (275) · `3` (198) · `4` (173) · `9` (24) |
| `grund_aenderung` | TEXT(50) | 0 | 2 | `Verbessert` (1,891) · `Real` (24) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (1,915) |
| `herkunft_jahr` | MEDIUMINT | 0 | 19 | `2019` (539) · `2024` (366) · `2020` (344) · `2021` (273) · `2022` (86) · `2023` (74) · `2010` (61) · `2018` (44) · `2009` (39) · `2008` (21) · `2013` (21) · `2014` (12) · `2011` (10) · `2017` (7) · `2016` (7) · `2025` (4) · `2015` (3) · `2007` (3) · `2012` (1) |
| `herkunft_monat` | MEDIUMINT | 0 | 8 | `6` (1,802) · `8` (25) · `7` (25) · `4` (23) · `5` (17) · `3` (15) · `2` (6) · `9` (2) |
| `objektart` | TEXT(50) | 0 | 4 | `Ausfahrt` (802) · `Einfahrt` (787) · `Verzweigung` (260) · `Ein- und Ausfahrt` (66) |
| `revision_jahr` | MEDIUMINT | 0 | 7 | `2024` (714) · `2023` (465) · `2019` (442) · `2025` (158) · `2022` (103) · `2020` (23) · `2021` (10) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (1,915) |
| `revision_qualitaet` | TEXT(50) | 4 | 2 | `Akt` (1,429) · `2020_Akt` (482) |
| `tlm_strassen_name_uuid` | TEXT(38) | 958 | >300 | _high cardinality — not enumerated_ |
| `name` | TEXT(254) | 958 | >300 | _high cardinality — not enumerated_ |
| `nummer` | TEXT(100) | 227 | 96 | `k_W` (787) · `2` (47) · `3` (43) · `6` (29) · `4` (27) · `5` (26) · `9` (25) · `1` (25) · `11` (23) · `8` (23) · `7` (23) · `14` (21) · `19` (20) · `10` (19) · `18` (19) · `12` (18) · `16` (17) · `17` (17) · `13` (16) · `33` (16) · `32` (16) · `37` (16) · `15` (15) · `34` (15) · `20` (14) · `28` (14) · `27` (14) · `36` (14) · `29` (13) · `30` (13) · `31` (13) · `35` (13) · `21` (12) · `22` (12) · `25` (12) · `38` (12) · `24` (11) · `26` (11) · `45` (10) · `42` (10) · … +56 more |

### `tlm_oev_schifffahrt`

*tlm_oev_schifffahrt*

**27** rows · geometry `geom` (LINESTRING, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | LINESTRING | 0 |  |  |
| `uuid` | TEXT(38) | 0 | 27 | `{DD54A04F-A872-4AFF-801B-6A2815519317}` (1) · `{510B7C72-C158-4B2C-942D-9F60A3A18764}` (1) · `{C6B2F273-69CC-4298-87CC-8BDD2A176114}` (1) · `{4F684DA2-B847-439A-8348-4BF6AD433281}` (1) · `{F24C08D8-723C-461F-AD93-D969C6B58E8A}` (1) · `{6A661DC4-73AE-45D9-BE7F-81D1D97A692A}` (1) · `{08F35FD6-DBD5-4F88-B983-389CC40600A5}` (1) · `{8B4BA4F3-E643-4286-BBAD-9F3B9C0D7659}` (1) · `{8C297A8A-BBCF-4379-BF9F-BA4F9E654EFB}` (1) · `{025D718D-FDD0-4056-8DFB-69CD5A7B759E}` (1) · `{35C3E853-452D-4B11-B3A0-A257DF32E615}` (1) · `{B09B692A-60D5-47CD-8A59-9FBA1BBEF0D6}` (1) · `{74414DC9-4317-4561-B82F-EBDF9008F353}` (1) · `{92C2D515-6C27-4423-BC9B-36AC01822421}` (1) · `{A549747E-C3B2-4FCE-B204-252DAC0BEFF3}` (1) · `{1F68D76D-AA55-4C66-8140-FE53F4AD537A}` (1) · `{F3D18A35-3417-42E6-BEFD-3BD1DCEBED91}` (1) · `{F7C32128-C4CF-4CEB-97F4-A87EEE18F3FD}` (1) · `{265FCAFD-EA19-458F-9975-770018BE2EB6}` (1) · `{4F40569C-310A-47E1-8D40-92197DCDD077}` (1) · `{F236D009-FA38-4553-AB1D-5653E4AA339C}` (1) · `{A233AE2D-2523-4C3C-B1C9-DB1D22DB661A}` (1) · `{91CF5270-2668-4DD3-AEF9-F9A7ED5E44C3}` (1) · `{74707C6F-C3A1-43E5-88D8-00BCED6BF5F5}` (1) · `{8916F99E-2AB7-408B-A1AB-0F7D47076039}` (1) · `{FF078D76-6408-4EF8-9005-9B20D1A7FD25}` (1) · `{8FE73957-EB88-4E29-BB78-3A3FB16BD9E0}` (1) |
| `datum_aenderung` | DATE | 0 | 25 | `2014-11-13` (2) · `2024-11-05` (2) · `2025-12-08` (1) · `2021-10-07` (1) · `2014-06-24` (1) · `2020-02-27` (1) · `2020-07-15` (1) · `2023-11-02` (1) · `2019-05-22` (1) · `2020-11-03` (1) · `2013-01-08` (1) · `2021-09-08` (1) · `2014-11-05` (1) · `2012-09-05` (1) · `2024-11-12` (1) · `2024-09-11` (1) · `2024-07-19` (1) · `2025-01-13` (1) · `2025-02-12` (1) · `2025-02-06` (1) · `2013-06-13` (1) · `2025-02-24` (1) · `2025-02-27` (1) · `2021-09-10` (1) · `2023-08-21` (1) |
| `datum_erstellung` | DATE | 0 | 8 | `2009-01-05` (12) · `2008-01-09` (9) · `2009-05-13` (1) · `2011-02-07` (1) · `2011-05-16` (1) · `2012-08-31` (1) · `2013-01-07` (1) · `2018-01-30` (1) |
| `erstellung_jahr` | MEDIUMINT | 0 | 10 | `2000` (8) · `2005` (5) · `2002` (3) · `2010` (3) · `2006` (2) · `2004` (2) · `1998` (1) · `2008` (1) · `2011` (1) · `2017` (1) |
| `erstellung_monat` | MEDIUMINT | 21 | 4 | `3` (2) · `6` (2) · `5` (1) · `4` (1) |
| `grund_aenderung` | TEXT(50) | 0 | 1 | `Verbessert` (27) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (27) |
| `herkunft_jahr` | MEDIUMINT | 0 | 10 | `2024` (10) · `2019` (4) · `2011` (3) · `2013` (2) · `2010` (2) · `2021` (2) · `2025` (1) · `2022` (1) · `2018` (1) · `2023` (1) |
| `herkunft_monat` | MEDIUMINT | 0 | 4 | `6` (23) · `3` (2) · `7` (1) · `5` (1) |
| `objektart` | TEXT(50) | 0 | 2 | `Personenfaehre` (24) · `Autofaehre` (3) |
| `revision_jahr` | MEDIUMINT | 0 | 4 | `2024` (17) · `2025` (5) · `2019` (4) · `2023` (1) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (27) |
| `revision_qualitaet` | TEXT(50) | 0 | 2 | `Akt` (23) · `2020_Akt` (4) |
| `tlm_oev_name_uuid` | TEXT(38) | 0 | 27 | `{9C06F47F-55E3-420E-A5C8-571516C4A554}` (1) · `{72413FE1-5F08-491F-A2AF-2E102AE346BA}` (1) · `{425C2D63-943A-4F50-85DE-3F959CDEFDFA}` (1) · `{3E2ADCF4-161E-42D5-A4DB-67670D5DECD4}` (1) · `{4AB19A4C-9FAC-4399-9333-5BD236A1E3F1}` (1) · `{AF455AEC-C55F-4580-8823-3EA9346793E3}` (1) · `{F5815BF7-767A-47C9-8895-781A046A4A6B}` (1) · `{8F1BCDA3-192A-4B2E-87FB-71BC0C9B1B38}` (1) · `{0760FE0F-F058-46D8-9679-95C3750A922A}` (1) · `{603B8447-55FA-4831-B2CB-9C263298F18E}` (1) · `{73199D89-0A79-45B7-A986-201552C5F270}` (1) · `{5BF2DA4D-B378-421A-A210-F9D827D8C749}` (1) · `{8AB80E0B-8912-4423-A00D-BFCEE325F23A}` (1) · `{2597F770-8474-4F29-B083-EA0931DD29AC}` (1) · `{907EB2F4-6F73-4DD1-B97C-5F2A25A14938}` (1) · `{76D12E5E-340D-4821-A512-CBF55438E335}` (1) · `{DBAF1221-7932-44AB-A0C6-B38432544F23}` (1) · `{1366A40D-0640-4F42-AC0F-77046A87EFC6}` (1) · `{BAF2499F-8382-4F5D-92FE-05F6A46173E2}` (1) · `{BBAB47FF-9045-4A98-9D89-3F557A64F3FD}` (1) · `{4B1A1C3B-BA1B-4FC0-B336-27B677F12BC4}` (1) · `{5D8112FB-E000-4F0F-94C7-536C528258CE}` (1) · `{23F9460E-649D-49BA-A5E4-0ADF16976C4A}` (1) · `{EB4F0896-B012-418E-A67F-F5F0CC50119E}` (1) · `{F8052B38-95ED-4007-B6F6-DB123B22C1B5}` (1) · `{B1CBE52D-FD89-4247-A0CB-D79B6511B1E0}` (1) · `{CC1FBB7E-8E71-4E61-8DD7-53EFA17C6DE5}` (1) |
| `name` | TEXT(254) | 0 | 27 | `Romanshorn-Friedrichshafen (D)` (1) · `Gertau-Degenau (Bischofszell)` (1) · `Kloster Fahr-Schlieren` (1) · `Ellikon-Nack` (1) · `Tössegg-Buchberg` (1) · `Schloss Laufen-Schlössli Wörth` (1) · `Beckenried-Gersau` (1) · `Insel Schwanau` (1) · `Horgen-Meilen` (1) · `Zurzach-Kadelburg` (1) · `Rotsee` (1) · `Full-Waldshut` (1) · `Mumpf-Säckingen (D)` (1) · `St. Johann Fähre "Ueli"` (1) · `Klingental Fähre "Vogel Gryff"` (1) · `Münster Fähre "Leu"` (1) · `St. Alban Fähre "Wild Maa"` (1) · `Wolfwil-Wynau` (1) · `Bodenacker (Muri bei Bern)-Nesslern (Kleinwabern)` (1) · `Zehendermätteli (Engehalbinsel)-Bremgarten` (1) · `Leuzigen-Altreu` (1) · `Paradies/Schlatt-Büsingen (D)` (1) · `Reichenbach (Unterzollikofen)-Engehalbinsel` (1) · `Scherzligen-Bächimatt` (1) · `Kaiseraugst-Herten` (1) · `Sulz-Fischbach` (1) · `Tariche` (1) |

### `tlm_areale_schutzgebiet`

*tlm_areale_schutzgebiet*

**2** rows · geometry `geom` (POLYGON, SRID 2056, has Z) · R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `geom` | POLYGON | 0 |  |  |
| `uuid` | TEXT(38) | 0 | 2 | `{41C8FE71-5621-4ECA-9AA3-2C7C8DF5D7C9}` (1) · `{57FE9E19-D6AD-4C64-9FDA-E0498BFFD60C}` (1) |
| `datum_aenderung` | DATE | 0 | 1 | `2016-12-15` (2) |
| `datum_erstellung` | DATE | 0 | 1 | `2014-11-03` (2) |
| `erstellung_jahr` | MEDIUMINT | 0 | 1 | `2014` (2) |
| `erstellung_monat` | MEDIUMINT | 0 | 1 | `8` (2) |
| `grund_aenderung` | TEXT(50) | 0 | 1 | `Verbessert` (2) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (2) |
| `herkunft_jahr` | MEDIUMINT | 0 | 1 | `2017` (2) |
| `herkunft_monat` | MEDIUMINT | 0 | 1 | `1` (2) |
| `name` | TEXT(254) | 0 | 1 | `Parc Naziunal Svizzer | Parc national suisse | Parco nazionale svizzero | Schweizerischer Nationalpark` (2) |
| `objektart` | TEXT(50) | 0 | 1 | `Nationalpark` (2) |
| `revision_jahr` | MEDIUMINT | 0 | 1 | `2026` (2) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `1` (2) |
| `revision_qualitaet` | TEXT(50) | 0 | 1 | `TGMG_2020_Akt` (2) |
| `tlm_grenzen_name_uuid` | TEXT(38) | 0 | 1 | `{4BA9966F-1B93-4860-ACDA-1A715E86649C}` (2) |

### `tlm_strassen_strassenname_strasse`

*tlm_strassen_strassenname_strasse*

**948,833** rows · no R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `tlm_strasse_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `tlm_strassenname_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |

### `tlm_strassen_strassenname`

*tlm_strassen_strassenname*

**173,860** rows · no R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | 176 | `2021-11-26` (15,718) · `2020-12-11` (13,159) · `2020-05-20` (11,342) · `2022-06-27` (10,894) · `2021-05-19` (10,756) · `2021-05-20` (8,951) · `2020-02-14` (7,780) · `2021-03-24` (6,938) · `2021-11-29` (6,635) · `2020-12-14` (6,437) · `2021-05-21` (6,121) · `2020-06-11` (5,994) · `2022-06-28` (4,131) · `2021-03-04` (4,129) · `2021-10-22` (4,095) · `2021-08-09` (3,986) · `2020-08-20` (3,533) · `2020-07-29` (3,410) · `2023-04-25` (3,076) · `2021-03-03` (3,041) · `2021-10-25` (3,012) · `2020-01-23` (2,545) · `2020-05-26` (2,462) · `2021-11-24` (2,433) · `2020-01-31` (2,286) · `2020-08-19` (2,129) · `2019-12-17` (2,005) · `2021-08-10` (1,966) · `2020-04-22` (1,849) · `2022-11-11` (1,532) · `2022-06-01` (1,241) · `2024-04-18` (1,209) · `2024-04-30` (1,131) · `2020-02-03` (967) · `2021-11-22` (718) · `2021-05-14` (584) · `2025-08-28` (495) · `2022-06-21` (363) · `2021-03-22` (343) · `2023-10-26` (324) · … +136 more |
| `datum_erstellung` | DATE | 0 | 156 | `2021-11-26` (15,752) · `2020-12-11` (13,308) · `2020-05-20` (11,648) · `2022-06-27` (10,894) · `2021-05-19` (10,759) · `2021-05-20` (8,953) · `2020-02-14` (7,913) · `2021-03-24` (7,139) · `2021-11-29` (6,641) · `2020-12-14` (6,449) · `2021-05-21` (6,123) · `2020-06-11` (6,044) · `2021-10-22` (4,141) · `2022-06-28` (4,131) · `2021-03-04` (4,130) · `2021-08-09` (4,055) · `2020-08-20` (3,534) · `2020-07-29` (3,416) · `2023-04-25` (3,076) · `2021-03-03` (3,041) · `2021-10-25` (3,016) · `2020-01-23` (2,545) · `2020-05-26` (2,499) · `2021-11-24` (2,431) · `2020-01-31` (2,286) · `2020-08-19` (2,130) · `2019-12-17` (2,006) · `2021-08-10` (1,997) · `2020-04-22` (1,849) · `2022-11-11` (1,531) · `2022-06-01` (1,250) · `2024-04-18` (1,209) · `2024-04-30` (1,131) · `2020-02-03` (967) · `2021-11-22` (724) · `2021-05-14` (584) · `2025-08-28` (495) · `2021-03-22` (348) · `2023-10-26` (324) · `2021-11-23` (268) · … +116 more |
| `erstellung_jahr` | MEDIUMINT | 0 | 9 | `2019` (63,395) · `2018` (60,630) · `2020` (38,411) · `2022` (4,564) · `2017` (2,125) · `2023` (2,091) · `2024` (1,418) · `2021` (1,204) · `2025` (22) |
| `erstellung_monat` | MEDIUMINT | 0 | 8 | `6` (166,808) · `4` (2,695) · `8` (1,725) · `7` (965) · `11` (592) · `10` (462) · `5` (321) · `1` (292) |
| `grund_aenderung` | TEXT(50) | 0 | 2 | `Verbessert` (173,853) · `Real` (7) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (161,889) · `Amtliches_Strassenverzeichnis` (11,971) |
| `herkunft_jahr` | MEDIUMINT | 0 | 9 | `2019` (63,338) · `2018` (60,183) · `2020` (38,394) · `2022` (4,626) · `2017` (2,124) · `2023` (2,091) · `2021` (1,663) · `2024` (1,419) · `2025` (22) |
| `herkunft_monat` | MEDIUMINT | 0 | 8 | `6` (166,844) · `4` (2,678) · `8` (1,714) · `7` (965) · `11` (590) · `10` (457) · `5` (321) · `1` (291) |
| `objektart` | TEXT(50) | 173,860 | 0 |  |
| `revision_jahr` | MEDIUMINT | 3,761 | 9 | `2024` (62,888) · `2019` (42,634) · `2023` (40,059) · `2025` (14,726) · `2022` (9,442) · `2020` (299) · `2017` (38) · `2021` (12) · `2018` (1) |
| `revision_monat` | MEDIUMINT | 3,761 | 1 | `6` (170,099) |
| `revision_qualitaet` | TEXT(50) | 39,013 | 4 | `Akt` (125,885) · `2020_Akt` (8,950) · `2021_Akt` (11) · `TLM_2022_RG_Akt` (1) |
| `name` | TEXT(254) | 0 | >300 | _high cardinality — not enumerated_ |
| `esid` | MEDIUMINT | 0 | >300 | _high cardinality — not enumerated_ |
| `kanton` | TEXT(2) | 1,222 | 26 | `ZH` (26,646) · `BE` (17,006) · `SG` (16,383) · `AG` (14,674) · `VD` (13,537) · `VS` (10,724) · `GR` (9,720) · `FR` (8,302) · `SO` (8,109) · `TI` (7,920) · `LU` (6,435) · `TG` (6,068) · `BL` (5,942) · `GE` (3,690) · `SZ` (2,685) · `NE` (2,536) · `JU` (2,185) · `AR` (2,121) · `SH` (1,908) · `BS` (1,388) · `ZG` (1,065) · `GL` (1,044) · `OW` (856) · `UR` (785) · `NW` (564) · `AI` (345) |
| `gdename` | TEXT(40) | 0 | >300 | _high cardinality — not enumerated_ |
| `gdenr` | SMALLINT | 0 | >300 | _high cardinality — not enumerated_ |
| `sprachcode` | TEXT(3) | 0 | 4 | `GER` (123,208) · `FRA` (37,517) · `ITA` (8,934) · `ROH` (4,201) |

### `tlm_strassen_strassenroute_strasse`

*tlm_strassen_strassenroute_strasse*

**158,559** rows · no R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `tlm_strasse_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `tlm_strassenroute_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |

### `tlm_areale_nutzungsareal_schule`

*tlm_areale_nutzungsareal_schule*

**7,864** rows · no R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `tlm_schule_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `tlm_nutzungsareal_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |

### `tlm_areale_schule`

*tlm_areale_schule*

**6,142** rows · no R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `erstellung_jahr` | MEDIUMINT | 0 | 13 | `2016` (4,853) · `2020` (228) · `2019` (202) · `2018` (168) · `2013` (119) · `2014` (113) · `2021` (110) · `2015` (107) · `2017` (91) · `2024` (80) · `2023` (31) · `2022` (30) · `2011` (10) |
| `erstellung_monat` | MEDIUMINT | 0 | 2 | `4` (4,698) · `6` (1,444) |
| `grund_aenderung` | TEXT(50) | 0 | 2 | `Verbessert` (6,141) · `Real` (1) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (6,142) |
| `herkunft_jahr` | MEDIUMINT | 0 | 13 | `2016` (3,381) · `2014` (360) · `2013` (346) · `2018` (339) · `2020` (338) · `2019` (319) · `2015` (250) · `2017` (236) · `2021` (214) · `2024` (143) · `2022` (109) · `2023` (87) · `2011` (20) |
| `herkunft_monat` | MEDIUMINT | 0 | 2 | `6` (3,129) · `4` (3,013) |
| `name` | TEXT(254) | 6,142 | 0 |  |
| `revision_jahr` | MEDIUMINT | 0 | 7 | `2024` (2,375) · `2023` (1,650) · `2019` (1,376) · `2025` (395) · `2022` (255) · `2020` (88) · `2021` (3) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (6,142) |
| `revision_qualitaet` | TEXT(50) | 36 | 2 | `Akt` (4,628) · `2020_Akt` (1,478) |
| `name_schule` | TEXT(254) | 0 | >300 | _high cardinality — not enumerated_ |
| `isced_stufe` | TEXT(50) | 0 | 6 | `600` (3,316) · `500` (1,475) · `400` (677) · `200` (537) · `999997` (129) · `300` (8) |
| `objectid` | TEXT(10) | 0 | >300 | _high cardinality — not enumerated_ |

### `tlm_bauten_leitung_stromtrasse`

*tlm_bauten_leitung_stromtrasse*

**2,979** rows · no R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `tlm_stromtrasse_uuid` | TEXT(38) | 0 | 3 | `{50A407E9-34D7-4CFA-BC88-2B1E03F4CD95}` (2,136) · `{FD856E23-D01E-4697-AF3B-1CBBA7D756C9}` (646) · `{DB81765A-79DC-4D94-8CFE-33A9F6069DD6}` (197) |
| `tlm_leitung_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |

### `tlm_bb_glamos`

*tlm_bb_glamos*

**2,038** rows · no R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | 35 | `2024-12-13` (658) · `2023-11-03` (372) · `2025-09-19` (317) · `2025-12-19` (230) · `2024-11-29` (139) · `2023-12-13` (109) · `2025-11-05` (54) · `2025-12-17` (43) · `2025-11-11` (34) · `2025-10-21` (15) · `2023-09-04` (15) · `2025-03-07` (6) · `2024-12-02` (5) · `2024-12-04` (4) · `2025-11-04` (4) · `2022-11-09` (3) · `2024-11-27` (3) · `2025-11-13` (3) · `2025-11-03` (3) · `2025-12-22` (3) · `2023-09-08` (2) · `2023-12-15` (2) · `2024-11-28` (2) · `2025-12-05` (1) · `2023-12-19` (1) · `2024-01-15` (1) · `2023-11-02` (1) · `2022-11-10` (1) · `2023-12-14` (1) · `2023-09-20` (1) · `2023-10-31` (1) · `2023-12-29` (1) · `2025-01-09` (1) · `2024-11-22` (1) · `2024-11-26` (1) |
| `datum_erstellung` | DATE | 87 | 62 | `2018-04-25` (1,294) · `2024-11-29` (136) · `2023-12-13` (92) · `2025-11-05` (65) · `2025-12-17` (39) · `2023-09-04` (29) · `2021-02-02` (27) · `2022-11-10` (27) · `2023-12-15` (26) · `2019-11-19` (17) · `2021-02-05` (15) · `2018-12-04` (15) · `2023-10-09` (14) · `2021-01-08` (12) · `2019-02-08` (11) · `2018-04-27` (9) · `2021-01-12` (7) · `2025-03-07` (6) · `2019-01-14` (6) · `2025-11-04` (6) · `2025-11-03` (6) · `2021-01-06` (5) · `2020-12-08` (5) · `2023-12-14` (5) · `2024-12-02` (5) · `2020-12-11` (4) · `2020-12-10` (4) · `2021-01-13` (4) · `2018-05-09` (4) · `2018-12-05` (4) · `2021-02-10` (4) · `2024-12-04` (3) · `2019-01-11` (3) · `2021-01-04` (3) · `2021-01-25` (3) · `2024-11-27` (3) · `2023-12-19` (2) · `2018-11-21` (2) · `2021-01-11` (2) · `2022-11-25` (2) · … +22 more |
| `erstellung_jahr` | MEDIUMINT | 87 | 15 | `2016` (615) · `2015` (301) · `2023` (184) · `2017` (177) · `2013` (111) · `2022` (110) · `2011` (95) · `2021` (90) · `2012` (79) · `2014` (48) · `2024` (48) · `2025` (47) · `2019` (26) · `2018` (19) · `2020` (1) |
| `erstellung_monat` | MEDIUMINT | 1,383 | 2 | `6` (630) · `1` (25) |
| `grund_aenderung` | TEXT(50) | 0 | 2 | `Verbessert` (2,035) · `Real` (3) |
| `herkunft` | TEXT(50) | 0 | 2 | `swisstopo` (2,026) · `SGI` (12) |
| `herkunft_jahr` | MEDIUMINT | 0 | 5 | `2023` (932) · `2022` (508) · `2024` (318) · `2025` (275) · `2021` (5) |
| `herkunft_monat` | MEDIUMINT | 1,227 | 2 | `6` (786) · `1` (25) |
| `objektart` | TEXT(50) | 2,038 | 0 |  |
| `revision_jahr` | MEDIUMINT | 0 | 4 | `2023` (797) · `2022` (504) · `2024` (453) · `2025` (284) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (2,038) |
| `revision_qualitaet` | TEXT(50) | 33 | 2 | `Akt` (1,945) · `2020_Akt` (60) |
| `tlm_bodenbedeckung_uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `sgi` | TEXT(100) | 0 | >300 | _high cardinality — not enumerated_ |
| `riverlevel0` | TEXT(5) | 1,013 | 16 | `f` (135) · `g` (126) · `e` (103) · `d` (102) · `m` (86) · `l` (85) · `i` (82) · `h` (73) · `b` (70) · `c` (47) · `j` (38) · `n` (37) · `a` (16) · `p` (12) · `k` (11) · `o` (2) |
| `riverlevel1` | MEDIUMINT | 0 | 9 | `4` (563) · `1` (351) · `5` (292) · `3` (222) · `2` (221) · `0` (172) · `6` (159) · `7` (36) · `8` (22) |
| `riverlevel2` | MEDIUMINT | 0 | 10 | `5` (986) · `1` (265) · `3` (162) · `8` (161) · `4` (139) · `7` (96) · `2` (80) · `9` (59) · `6` (52) · `0` (38) |
| `riverlevel3` | TEXT(5) | 0 | 4 | `A` (1,027) · `B` (764) · `E` (138) · `C` (109) |
| `inventorycode` | MEDIUMINT | 0 | 83 | `4` (146) · `2` (138) · `3` (118) · `6` (112) · `5` (106) · `7` (103) · `8` (84) · `11` (79) · `1` (75) · `10` (75) · `9` (63) · `15` (62) · `12` (59) · `13` (59) · `14` (58) · `16` (50) · `18` (40) · `19` (39) · `17` (38) · `22` (36) · `24` (31) · `21` (30) · `20` (28) · `23` (27) · `27` (26) · `25` (24) · `26` (20) · `32` (16) · `31` (16) · `37` (14) · `28` (14) · `29` (13) · `33` (13) · `35` (12) · `34` (11) · `30` (11) · `38` (10) · `51` (10) · `36` (9) · `44` (9) · … +43 more |

### `tlm_strassen_strassenroute`

*tlm_strassen_strassenroute*

**535** rows · no R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `uuid` | TEXT(38) | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_aenderung` | DATE | 0 | >300 | _high cardinality — not enumerated_ |
| `datum_erstellung` | DATE | 0 | 79 | `2010-06-24` (178) · `2010-06-25` (80) · `2010-06-23` (42) · `2009-07-28` (20) · `2010-04-14` (15) · `2010-06-22` (14) · `2011-10-20` (13) · `2019-11-21` (10) · `2009-01-14` (8) · `2009-06-12` (8) · `2010-02-26` (8) · `2008-09-11` (7) · `2009-11-16` (7) · `2010-02-24` (7) · `2009-09-29` (6) · `2022-12-05` (6) · `2009-07-29` (4) · `2010-02-25` (4) · `2010-04-12` (4) · `2022-11-14` (4) · `2008-09-04` (3) · `2008-12-03` (3) · `2009-04-22` (3) · `2009-06-16` (3) · `2009-06-24` (3) · `2011-10-18` (3) · `2024-06-14` (3) · `2008-09-17` (2) · `2008-12-11` (2) · `2009-01-13` (2) · `2009-04-23` (2) · `2009-04-24` (2) · `2009-06-15` (2) · `2009-11-17` (2) · `2009-11-19` (2) · `2010-02-23` (2) · `2010-04-16` (2) · `2010-11-02` (2) · `2011-09-20` (2) · `2022-03-24` (2) · … +39 more |
| `erstellung_jahr` | MEDIUMINT | 0 | 10 | `2009` (383) · `2008` (94) · `2019` (13) · `2020` (13) · `2021` (12) · `2010` (10) · `2024` (4) · `2013` (2) · `2018` (2) · `2023` (2) |
| `erstellung_monat` | MEDIUMINT | 0 | 6 | `4` (331) · `5` (94) · `6` (78) · `7` (24) · `8` (6) · `9` (2) |
| `grund_aenderung` | TEXT(50) | 0 | 2 | `Verbessert` (497) · `Real` (38) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (535) |
| `herkunft_jahr` | MEDIUMINT | 0 | 18 | `2024` (135) · `2023` (89) · `2022` (69) · `2021` (59) · `2025` (52) · `2020` (34) · `2019` (24) · `2018` (16) · `2013` (10) · `2017` (7) · `2016` (7) · `2012` (7) · `2011` (6) · `2014` (5) · `2008` (4) · `2015` (4) · `2010` (4) · `2009` (3) |
| `herkunft_monat` | MEDIUMINT | 0 | 6 | `6` (508) · `4` (11) · `3` (11) · `5` (3) · `7` (1) · `8` (1) |
| `objektart` | TEXT(50) | 0 | 9 | `Hauptstrasse B` (394) · `Nationalstrasse` (35) · `Hauptstrasse A` (32) · `HLS Bund` (31) · `HLS Kanton` (19) · `Hauptstrasse C` (12) · `Europastrasse` (10) · `Hauptstrasse swisstopo rot` (1) · `Hauptstrasse swisstopo gelb` (1) |
| `revision_jahr` | MEDIUMINT | 0 | 7 | `2024` (234) · `2023` (123) · `2025` (89) · `2022` (57) · `2019` (25) · `2020` (5) · `2021` (2) |
| `revision_monat` | MEDIUMINT | 0 | 2 | `6` (533) · `4` (2) |
| `revision_qualitaet` | TEXT(50) | 11 | 2 | `Akt` (449) · `2020_Akt` (75) |
| `name` | TEXT(254) | 0 | >300 | _high cardinality — not enumerated_ |
| `routennummer` | TEXT(10) | 14 | >300 | _high cardinality — not enumerated_ |

### `tlm_bauten_stromtrasse`

*tlm_bauten_stromtrasse*

**3** rows · no R-tree index

| Column | Type | Null | Distinct | Values |
|---|---|---:|---:|---|
| `id` | INTEGER | 0 |  |  |
| `uuid` | TEXT(38) | 0 | 3 | `{DB81765A-79DC-4D94-8CFE-33A9F6069DD6}` (1) · `{50A407E9-34D7-4CFA-BC88-2B1E03F4CD95}` (1) · `{FD856E23-D01E-4697-AF3B-1CBBA7D756C9}` (1) |
| `datum_aenderung` | DATE | 0 | 1 | `2022-09-23` (3) |
| `datum_erstellung` | DATE | 0 | 1 | `2022-09-23` (3) |
| `erstellung_jahr` | MEDIUMINT | 0 | 1 | `2021` (3) |
| `erstellung_monat` | MEDIUMINT | 0 | 1 | `6` (3) |
| `grund_aenderung` | TEXT(50) | 0 | 1 | `Verbessert` (3) |
| `herkunft` | TEXT(50) | 0 | 1 | `swisstopo` (3) |
| `herkunft_jahr` | MEDIUMINT | 0 | 1 | `2021` (3) |
| `herkunft_monat` | MEDIUMINT | 0 | 1 | `6` (3) |
| `name` | TEXT(254) | 3 | 0 |  |
| `revision_jahr` | MEDIUMINT | 0 | 1 | `2025` (3) |
| `revision_monat` | MEDIUMINT | 0 | 1 | `6` (3) |
| `revision_qualitaet` | TEXT(50) | 0 | 1 | `Akt` (3) |
| `netzebene` | TEXT(50) | 0 | 3 | `999997` (1) · `3` (1) · `1` (1) |
