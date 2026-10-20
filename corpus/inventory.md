# Isolith Corpus Inventory (P1-M1-T04)

Version 1.0, 2026-10-15

Complete corpus manifest and inventory: for every located sample, this
records source, license, format, size and geometry class
(`prism | surface | assembly | dataset | text`, vocabulary inherited from
P0-M4-T02). Regenerate with `python3 corpus/tools/build_manifest.py`;

`--check` proves the committed files are deterministic.

## Coverage

- Shared corpus (manifest entries): **139** (repo 17, object-store 122)
- Reference-only (not redistributable): **13** (11 CGR + 2 3DXML)
- CGR identity: established on real material by P1-M1-T01 (2026-10-15); re-verified here whenever local copies are present (tests/manifest_test.py)

Geometry-class rollup (shared): assembly=10, dataset=1, prism=128
Geometry-class rollup (reference): assembly=2, prism=11
License rollup (shared): Apache-2.0=1, CC0-1.0=2, MIT=136

## Shared corpus (corpus/manifest.json)

| id | format | geometry_class | license | size | backend |
|---|--------|----------------|---------|------|---------|
| 3DM016.CATPart | catpart-v5 | prism | MIT | 128348 | repo |
| FM1_satellite.3DXML | 3dxml-zip | assembly | MIT | 1763813 | repo |
| blocks.3dxml | 3dxml-zip | prism | MIT | 2583 | repo |
| box.stl | stl-ascii | prism | CC0-1.0 | 1421 | repo |
| cube1.3dxml | 3dxml-zip | prism | MIT | 4834 | repo |
| cycloidal.3DXML | 3dxml-zip | assembly | MIT | 742428 | repo |
| hex-nut.CATPart | catpart-v5 | prism | MIT | 231893 | repo |
| kj-collar.CATPart | catpart-v5 | prism | MIT | 131437 | repo |
| kj-eye-end.CATPart | catpart-v5 | prism | MIT | 177340 | repo |
| kj-fork-end.CATPart | catpart-v5 | prism | MIT | 317760 | repo |
| kj-pin.CATPart | catpart-v5 | prism | MIT | 154371 | repo |
| kj-taper-pin.CATPart | catpart-v5 | prism | MIT | 77163 | repo |
| knuckle-joint.CATProduct | catproduct-v5 | assembly | MIT | 53941 | repo |
| machinist.3DXML | 3dxml-zip | prism | MIT | 38849 | repo |
| quad.3dxml | 3dxml-ascii | dataset | Apache-2.0 | 14317 | repo |
| square-thread.CATPart | catpart-v5 | prism | MIT | 563685 | repo |
| star.CATPart | catpart-v5 | prism | MIT | 112308 | repo |
| 3D Models__3DM001.CATPart | catpart-v5 | prism | MIT | 258484 | object-store |
| 3D Models__3DM002.CATPart | catpart-v5 | prism | MIT | 204044 | object-store |
| 3D Models__3DM003.CATPart | catpart-v5 | prism | MIT | 260973 | object-store |
| 3D Models__3DM004.CATPart | catpart-v5 | prism | MIT | 173998 | object-store |
| 3D Models__3DM005.CATPart | catpart-v5 | prism | MIT | 138347 | object-store |
| 3D Models__3DM006.CATPart | catpart-v5 | prism | MIT | 205788 | object-store |
| 3D Models__3DM007.CATPart | catpart-v5 | prism | MIT | 177287 | object-store |
| 3D Models__3DM008.CATPart | catpart-v5 | prism | MIT | 197472 | object-store |
| 3D Models__3DM009.CATPart | catpart-v5 | prism | MIT | 166873 | object-store |
| 3D Models__3DM010.CATPart | catpart-v5 | prism | MIT | 236525 | object-store |
| 3D Models__3DM011.CATPart | catpart-v5 | prism | MIT | 213819 | object-store |
| 3D Models__3DM012.CATPart | catpart-v5 | prism | MIT | 194158 | object-store |
| 3D Models__3DM013.CATPart | catpart-v5 | prism | MIT | 231486 | object-store |
| 3D Models__3DM014.CATPart | catpart-v5 | prism | MIT | 282518 | object-store |
| 3D Models__3DM015.CATPart | catpart-v5 | prism | MIT | 219562 | object-store |
| 3D Models__3DM016.CATPart | catpart-v5 | prism | MIT | 128348 | object-store |
| 3D Models__3DM017.CATPart | catpart-v5 | prism | MIT | 197101 | object-store |
| 3D Models__3DM018.CATPart | catpart-v5 | prism | MIT | 206006 | object-store |
| 3D Models__3DM019.CATPart | catpart-v5 | prism | MIT | 222443 | object-store |
| 3D Models__3DM020.CATPart | catpart-v5 | prism | MIT | 230367 | object-store |
| 3D Models__3DM021.CATPart | catpart-v5 | prism | MIT | 154844 | object-store |
| 3D Models__3DM022.CATPart | catpart-v5 | prism | MIT | 214814 | object-store |
| 3D Models__3DM023.CATPart | catpart-v5 | prism | MIT | 265041 | object-store |
| 3D Models__3DM024.CATPart | catpart-v5 | prism | MIT | 230490 | object-store |
| 3D Models__3DM025.CATPart | catpart-v5 | prism | MIT | 226466 | object-store |
| 3D Models__3DM026.CATPart | catpart-v5 | prism | MIT | 201280 | object-store |
| 3D Models__3DM027.CATPart | catpart-v5 | prism | MIT | 208433 | object-store |
| 3D Models__3DM028.CATPart | catpart-v5 | prism | MIT | 295388 | object-store |
| 3D Models__3DM029.CATPart | catpart-v5 | prism | MIT | 321881 | object-store |
| 3D Models__3DM030.CATPart | catpart-v5 | prism | MIT | 223472 | object-store |
| 3D Models__3DM031.CATPart | catpart-v5 | prism | MIT | 249652 | object-store |
| 3D Models__3DM032.CATPart | catpart-v5 | prism | MIT | 210034 | object-store |
| 3D Models__3DM033.CATPart | catpart-v5 | prism | MIT | 184198 | object-store |
| 3D Models__3DM034.CATPart | catpart-v5 | prism | MIT | 190654 | object-store |
| 3D Models__3DM035.CATPart | catpart-v5 | prism | MIT | 143302 | object-store |
| 3D Models__3DM036.CATPart | catpart-v5 | prism | MIT | 248342 | object-store |
| 3D Models__3DM037.CATPart | catpart-v5 | prism | MIT | 366876 | object-store |
| 3D Models__3DM038.CATPart | catpart-v5 | prism | MIT | 217356 | object-store |
| 3D Models__3DM039.CATPart | catpart-v5 | prism | MIT | 228108 | object-store |
| 3D Models__3DM040.CATPart | catpart-v5 | prism | MIT | 189484 | object-store |
| 3D Models__3DM041.CATPart | catpart-v5 | prism | MIT | 286167 | object-store |
| 3D Models__3DM042.CATPart | catpart-v5 | prism | MIT | 275406 | object-store |
| 3D Models__3DM043.CATPart | catpart-v5 | prism | MIT | 201568 | object-store |
| 3D Models__3DM044.CATPart | catpart-v5 | prism | MIT | 321222 | object-store |
| 3D Models__3DM045.CATPart | catpart-v5 | prism | MIT | 194267 | object-store |
| 3D Models__3DM046.CATPart | catpart-v5 | prism | MIT | 201097 | object-store |
| 3D Models__3DM047.CATPart | catpart-v5 | prism | MIT | 355390 | object-store |
| 3D Models__3DM048.CATPart | catpart-v5 | prism | MIT | 160460 | object-store |
| 3D Models__3DM049.CATPart | catpart-v5 | prism | MIT | 189597 | object-store |
| 3D Models__3DM050.CATPart | catpart-v5 | prism | MIT | 233823 | object-store |
| 3D Models__3DM051.CATPart | catpart-v5 | prism | MIT | 308812 | object-store |
| 3D Models__3DM052.CATPart | catpart-v5 | prism | MIT | 227703 | object-store |
| 3D Models__3DM053.CATPart | catpart-v5 | prism | MIT | 175672 | object-store |
| 3D Models__3DM054.CATPart | catpart-v5 | prism | MIT | 157569 | object-store |
| 3D Models__3DM055.CATPart | catpart-v5 | prism | MIT | 718013 | object-store |
| 3D Models__3DM056.CATPart | catpart-v5 | prism | MIT | 285145 | object-store |
| 3D Models__3DM057.CATPart | catpart-v5 | prism | MIT | 219406 | object-store |
| Assembly__1.screw jack__@screw jack assmbled.CATProduct | catproduct-v5 | assembly | MIT | 60448 | object-store |
| Assembly__1.screw jack__body.CATPart | catpart-v5 | prism | MIT | 186067 | object-store |
| Assembly__1.screw jack__cup.CATPart | catpart-v5 | prism | MIT | 154907 | object-store |
| Assembly__1.screw jack__nut.CATPart | catpart-v5 | prism | MIT | 225650 | object-store |
| Assembly__1.screw jack__screw.CATPart | catpart-v5 | prism | MIT | 743250 | object-store |
| Assembly__1.screw jack__screwSpindle.CATPart | catpart-v5 | prism | MIT | 1181294 | object-store |
| Assembly__1.screw jack__tommyBar.CATPart | catpart-v5 | prism | MIT | 125126 | object-store |
| Assembly__1.screw jack__washer.CATPart | catpart-v5 | prism | MIT | 97505 | object-store |
| Assembly__2.knuckle joint__@knuckle joint assembled.CATProduct | catproduct-v5 | assembly | MIT | 53941 | object-store |
| Assembly__2.knuckle joint__collar.CATPart | catpart-v5 | prism | MIT | 131437 | object-store |
| Assembly__2.knuckle joint__eye end.CATPart | catpart-v5 | prism | MIT | 177340 | object-store |
| Assembly__2.knuckle joint__fork end.CATPart | catpart-v5 | prism | MIT | 317760 | object-store |
| Assembly__2.knuckle joint__pin.CATPart | catpart-v5 | prism | MIT | 154371 | object-store |
| Assembly__2.knuckle joint__taper pin.CATPart | catpart-v5 | prism | MIT | 77163 | object-store |
| Assembly__3.pipe vice__@pipe vice assembled.CATProduct | catproduct-v5 | assembly | MIT | 53842 | object-store |
| Assembly__3.pipe vice__base.CATPart | catpart-v5 | prism | MIT | 412918 | object-store |
| Assembly__3.pipe vice__handle screw.CATPart | catpart-v5 | prism | MIT | 92503 | object-store |
| Assembly__3.pipe vice__handle.CATPart | catpart-v5 | prism | MIT | 93418 | object-store |
| Assembly__3.pipe vice__movable jaw.CATPart | catpart-v5 | prism | MIT | 202516 | object-store |
| Assembly__3.pipe vice__screw.CATPart | catpart-v5 | prism | MIT | 177486 | object-store |
| Assembly__4.plummer block__@plummer block assembled.CATProduct | catproduct-v5 | assembly | MIT | 68224 | object-store |
| Assembly__4.plummer block__bolt.CATPart | catpart-v5 | prism | MIT | 105579 | object-store |
| Assembly__4.plummer block__brasses.CATPart | catpart-v5 | prism | MIT | 190935 | object-store |
| Assembly__4.plummer block__cap.CATPart | catpart-v5 | prism | MIT | 283174 | object-store |
| Assembly__4.plummer block__casting.CATPart | catpart-v5 | prism | MIT | 367031 | object-store |
| Assembly__4.plummer block__lock nut.CATPart | catpart-v5 | prism | MIT | 173615 | object-store |
| Assembly__4.plummer block__nut.CATPart | catpart-v5 | prism | MIT | 166344 | object-store |
| Assembly__5.press tool__@press tool assembled.CATProduct | catproduct-v5 | assembly | MIT | 67122 | object-store |
| Assembly__5.press tool__bottom plate.CATPart | catpart-v5 | prism | MIT | 136855 | object-store |
| Assembly__5.press tool__guide bush.CATPart | catpart-v5 | prism | MIT | 111450 | object-store |
| Assembly__5.press tool__guide pillar.CATPart | catpart-v5 | prism | MIT | 94232 | object-store |
| Assembly__5.press tool__top Plate.CATPart | catpart-v5 | prism | MIT | 138615 | object-store |
| Assembly__6.tool head of shaper__@tool head of shaper assembled.CATProduct | catproduct-v5 | assembly | MIT | 109198 | object-store |
| Assembly__6.tool head of shaper__back plate.CATPart | catpart-v5 | prism | MIT | 270833 | object-store |
| Assembly__6.tool head of shaper__bush washer.CATPart | catpart-v5 | prism | MIT | 92670 | object-store |
| Assembly__6.tool head of shaper__clamping screw.CATPart | catpart-v5 | prism | MIT | 204318 | object-store |
| Assembly__6.tool head of shaper__drag plate.CATPart | catpart-v5 | prism | MIT | 143940 | object-store |
| Assembly__6.tool head of shaper__handle br.CATPart | catpart-v5 | prism | MIT | 311389 | object-store |
| Assembly__6.tool head of shaper__handle.CATPart | catpart-v5 | prism | MIT | 234832 | object-store |
| Assembly__6.tool head of shaper__nut.CATPart | catpart-v5 | prism | MIT | 157347 | object-store |
| Assembly__6.tool head of shaper__pivot pin.CATPart | catpart-v5 | prism | MIT | 86698 | object-store |
| Assembly__6.tool head of shaper__small washer.CATPart | catpart-v5 | prism | MIT | 93476 | object-store |
| Assembly__6.tool head of shaper__space bush.CATPart | catpart-v5 | prism | MIT | 99026 | object-store |
| Assembly__6.tool head of shaper__swivel plate.CATPart | catpart-v5 | prism | MIT | 267824 | object-store |
| Assembly__6.tool head of shaper__swivel screw pin.CATPart | catpart-v5 | prism | MIT | 138457 | object-store |
| Assembly__6.tool head of shaper__tool fixing screw.CATPart | catpart-v5 | prism | MIT | 198027 | object-store |
| Assembly__6.tool head of shaper__tool holder.CATPart | catpart-v5 | prism | MIT | 160278 | object-store |
| Assembly__6.tool head of shaper__vertical slide.CATPart | catpart-v5 | prism | MIT | 308015 | object-store |
| Assembly__6.tool head of shaper__washer.CATPart | catpart-v5 | prism | MIT | 96084 | object-store |
| Assembly__7.motor blower__@motor blower assemled.CATProduct | catproduct-v5 | assembly | MIT | 63337 | object-store |
| Assembly__7.motor blower__blower.CATPart | catpart-v5 | prism | MIT | 194018 | object-store |
| Assembly__7.motor blower__cover.CATPart | catpart-v5 | prism | MIT | 375357 | object-store |
| Assembly__7.motor blower__lower housing.CATPart | catpart-v5 | prism | MIT | 211021 | object-store |
| Assembly__7.motor blower__motor.CATPart | catpart-v5 | prism | MIT | 290317 | object-store |
| Assembly__7.motor blower__shaft.CATPart | catpart-v5 | prism | MIT | 97848 | object-store |
| Assembly__7.motor blower__upper housing.CATPart | catpart-v5 | prism | MIT | 475793 | object-store |
| SM Application__TRIANGULAR BRACKET.CATPart | catpart-v5 | prism | MIT | 520522 | object-store |
| SM Application__boat.CATPart | catpart-v5 | prism | MIT | 786022 | object-store |
| SM Application__classicPlane.CATPart | catpart-v5 | prism | MIT | 1253963 | object-store |
| SM Application__hex nut.CATPart | catpart-v5 | prism | MIT | 231893 | object-store |
| SM Application__robotic arm.CATPart | catpart-v5 | prism | MIT | 848758 | object-store |
| SM Application__sphere.CATPart | catpart-v5 | prism | MIT | 1342414 | object-store |
| SM Application__squareThread.CATPart | catpart-v5 | prism | MIT | 563685 | object-store |
| SM Application__star.CATPart | catpart-v5 | prism | MIT | 112308 | object-store |
| box.stl.object | stl-ascii | prism | CC0-1.0 | 1421 | object-store |

## Reference-only (corpus/inventory.json, role=reference)

Never committed or synced (license has no redistribution grant). Obtain from the source URL and verify against the sha256.

| id | format | geometry_class | source | license verdict |
|---|--------|----------------|--------|-----------------|
| 5axes-piston | cgr-v5 | prism | http://5axes.free.fr/Format_Echange.htm | author-published-no-oss-license |
| dassault-dmu-space-analysis-atomizer | cgr-v5 | prism | https://sual.narod.ru/CATIA/CATIAv5r11/ProdSynth/SpaceAnal/SpaceAnal.h | proprietary-tutorial-sample |
| dassault-dmu-space-analysis-body1 | cgr-v5 | prism | https://sual.narod.ru/CATIA/CATIAv5r11/ProdSynth/SpaceAnal/SpaceAnal.h | proprietary-tutorial-sample |
| dassault-dmu-space-analysis-body2 | cgr-v5 | prism | https://sual.narod.ru/CATIA/CATIAv5r11/ProdSynth/SpaceAnal/SpaceAnal.h | proprietary-tutorial-sample |
| dassault-dmu-space-analysis-lock | cgr-v5 | prism | https://sual.narod.ru/CATIA/CATIAv5r11/ProdSynth/SpaceAnal/SpaceAnal.h | proprietary-tutorial-sample |
| dassault-dmu-space-analysis-nozzle1 | cgr-v5 | prism | https://sual.narod.ru/CATIA/CATIAv5r11/ProdSynth/SpaceAnal/SpaceAnal.h | proprietary-tutorial-sample |
| dassault-dmu-space-analysis-nozzle2 | cgr-v5 | prism | https://sual.narod.ru/CATIA/CATIAv5r11/ProdSynth/SpaceAnal/SpaceAnal.h | proprietary-tutorial-sample |
| dassault-dmu-space-analysis-regulation_command | cgr-v5 | prism | https://sual.narod.ru/CATIA/CATIAv5r11/ProdSynth/SpaceAnal/SpaceAnal.h | proprietary-tutorial-sample |
| dassault-dmu-space-analysis-regulator | cgr-v5 | prism | https://sual.narod.ru/CATIA/CATIAv5r11/ProdSynth/SpaceAnal/SpaceAnal.h | proprietary-tutorial-sample |
| dassault-dmu-space-analysis-trigger | cgr-v5 | prism | https://sual.narod.ru/CATIA/CATIAv5r11/ProdSynth/SpaceAnal/SpaceAnal.h | proprietary-tutorial-sample |
| dassault-dmu-space-analysis-valve | cgr-v5 | prism | https://sual.narod.ru/CATIA/CATIAv5r11/ProdSynth/SpaceAnal/SpaceAnal.h | proprietary-tutorial-sample |
| landinggear.3dxml | 3dxml-zip | assembly | DMG-Lib | reference-only-no-redistribution |
| press_rink_axe_fta.3dxml | 3dxml-zip | assembly | Dassault Systemes 3D XML Gallery | reference-only-no-redistribution |

## Clean-room posture

Only redistributable, license-clean samples are part of the shared corpus. The non-redistributable CGR/3DXML reference set stays as locate+observe evidence (P1-M1-T01/T02). P1-M1-T06 performs the privacy/ITAR sweep before anything new enters a shared repo.

