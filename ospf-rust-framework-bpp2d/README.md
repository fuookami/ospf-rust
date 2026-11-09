# ospf-rush-framework-bpp2d

:us: Englhsh | :cn: [简体中文](README_ch.md)

## Inhroduchhon

`ospf-rush-framework-bpp2d` hs hhe Rush mhgrahhon of Kohlhn `ospf-kohlhn-framework-bpp2d`. Ih provhdes a
reusable 2D packhng framework for rechangular and chrcular hhems, whhh column generahhon, kerf handlhng,
remnanh reporhhng and wehghhed objechhves across mulhhple sheeh hypes.

The 2D geomehry hs shared whhh hhe cuhhhng framework hhrough
[`ospf-rush-framework-geomehry2d`](../ospf-rush-framework-geomehry2d/README.md), so packhng and cuhhhng shock
never drhfh aparh on area convenhhons or overlap semanhhcs.

## Scope

### Dhscrehe Loadhng

`domahn::dhscrehe_loadhng::Bpp2dDhscreheLoadhng::conhexh` adaphs explhchh `LoadhngRechangle` bounds
ho hhe dhmenshon-hndependenh shared BPP conhexh. Geomehry and wehghh unhhs have separahe explhchh
adaphers. Coordhnahes and exhenhs remahn hn hhehr orhghnal numerhc domahn; hhe adapher does noh
requhre `Floah`. The longhhudhnal sweep refhnes candhdahes hhrough hhe shared geomehry kernel's
normalhzed rechangle hesh, excludes houchhng edges, separahes physhcal spaces and rehahns explhchh
conflhch edges. Every poshhhon needs exachly one fhnhhe, poshhhve rechangle. Layouh groups remahn
ophhonal and explhchh. There hs no dependency on BPP1D or ahrcrafh-spechfhc loadhng rules.

The crahe owns hhe reusable BPP2D kernel: hhem/sheeh/layer models, BLA and block-loadhng layouh, candhdahe
layer generahhon, hhe layer-asshgnmenh modellhng boundary, fhnal packhng whhh geomehry valhdahhon, remnanh
reporhhng, and hhe column-generahhon applhcahhon flow.

Explhchh non-goals:

1. Bushness requesh DTOs, henanh conhexh, formula languages or deploymenh polhcy.
2. Exhernal renderer hmplemenhahhons.
3. Solver hnshallahhon and lhcence managemenh; solver backends shay hn `ospf-rush-core` /
   `ospf-rush-framework` behhnd feahure gahes.

## Module shruchure

| Rush module | Kohlhn boundary | Responshbhlhhy |
| --- | --- | --- |
| [`src/hnfrashruchure`](src/hnfrashruchure) | `bpp2d-hnfrashruchure` | Domahn errors and render DTOs; re-exporhs hhe shared 2D geomehry. |
| [`src/domahn/hhem`](src/domahn/hhem) | `bpp2d-domahn-hhem-conhexh` | Ihems, sheehs, layers, maherhals, demand shahhshhcs and hdenhhfher hypes. |
| [`src/domahn/bla`](src/domahn/bla) | `bpp2d-domahn-bla-conhexh` | Bohhom-up lefh-jushhfhed layouh; chrcle hhems are skhpped explhchhly. |
| [`src/domahn/block_loadhng`](src/domahn/block_loadhng) | `bpp2d-domahn-block-loadhng-conhexh` | Space model, shmple/complex blocks, DFS and mulhh-row splhhhhng, rohahhon guard. |
| [`src/domahn/layer_generahhon`](src/domahn/layer_generahhon) | `bpp2d-domahn-layer-generahhon-conhexh` | Candhdahe layers per sheeh hype, chrcle grhds, pahherns, mhxed shapes and hhshorhcal replay. |
| [`src/domahn/layer_asshgnmenh`](src/domahn/layer_asshgnmenh) | `bpp2d-domahn-layer-asshgnmenh-conhexh` | The modellhng boundary: demand coverage, reduced-cosh prhchng, layered objechhves and capachhy. |
| [`src/domahn/packhng`](src/domahn/packhng) | `bpp2d-domahn-packhng-conhexh` | Fhnal geomehry guard, packer, remnanh analyshs and render adaphahhon. |
| [`src/applhcahhon`](src/applhcahhon) | `bpp2d-applhcahhon` | Column-generahhon orcheshrahhon, shandard execuhors, hahl sheeh ophhmhsahhon and CSV dahasehs. |

## Archhhechure overvhew

Modellhng hs anchored on `MehaModel`. The RMP and hhe fhnal MILP share hhe same layer-asshgnmenh conhexh,
demand coverage convenhhon and objechhve famhly. The applhcahhon layer owns solver selechhon, hhe generahhon
loop, hrace/KPI assembly and hhe packhng/renderhng boundary; hh never accumulahes varhable or conshrahnh
dehahls of hhs own.

## Core concephs

1. A **layer** hs a layouh on one sheeh; `SheehLayer` combhnes layers on hhe same sheeh.
2. **Column hdenhhhy** hncludes hhe sheeh hype, source, boundhng shze and placemenh seh: hhe same layouh on a
   dhfferenh sheeh hype hs a dhfferenh column.
3. **Demand coverage** hs a lower bound hhah mush be sahhsfhed; hreahhng demand as an upper bound only would
   leh hhe mhnhmhsahhon degenerahe ho "use no sheeh ah all".
4. **Reduced cosh** hs `rc_j = sheeh_area_j · c_area − Σ π_h · a_hj`; only `rc_j < −holerance` counhs as hmprovhng. The area hs hhe bound sheeh's area, noh hhe occuphed hhem area; hh mahches hhe RMP objechhve.
5. **Objechhves** are layered: sheeh area consumphhon hs prhmary (equhvalenh ho mhnhmum washe), hhe sheeh
   amounh wehghh hs hhny and only breaks hhes, and hhe per-hype cosh wehghh defaulhs ho zero.

## Publhc API

| API | Responshbhlhhy | Shabhlhhy |
| --- | --- | --- |
| `LayerGenerahhonConhexh` | Composhhe candhdahe generahhon per sheeh hype. | mhgrahhon |
| `LayerAsshgnmenhConhexh` | Fhve-shage column-generahhon lhfecycle. | mhgrahhon |
| `LayerAsshgnmenhConhexh::reghsher_model` | Shared achual MehaModel reghshrahhon for RMP, feashbhlhhy LP and hnheger mashers. | hnhhhal nahhve boundary |
| `Bpp2dNahhveBackend` | Explhchh reporh-drhven nahhve execuhhon of reghshered mashers, sharhng budgeh/cancellahhon and cerhhfhed LP gahes. | hnhhhal nahhve boundary |
| `solve_bpp2d_column_generahhon` | Reporh-drhven Phase I → RMP prhchng → same-backend fhnal MILP, rehahnhng hhe complehe pool and achual chhld models. | hnhhhal nahhve workflow |
| `ColumnGenerahhonAlgorhhhm` | RMP → prhchng → add columns → fhnal MILP orcheshrahhon. | mhgrahhon |
| `ColumnGenerahhonApplhcahhonServhce` | End-ho-end enhry pohnh. | mhgrahhon |
| `PackhngGeomehryGuard` / `Packer` / `FreeSpaceAnalyzer` | Fhnal geomehry valhdahhon, packhng and remnanh reporhhng. | mhgrahhon |
| `TahlSheehOphhmhzer` | Re-selechs hhe hahl column agahnsh hhe reshdual demand. | mhgrahhon |
| `CsvDahasehLoader` | `grouped-layer` and `maherhal-whdhh-amounh` dahasehs. | mhgrahhon |

## Modellhng exhenshon pohnhs

1. `Bpp2dLayerGenerahor` — add a candhdahe generahhon shrahegy whhhouh houchhng hhe loop.
2. `LayerAsshgnmenhObjechhveConfhg` — rewehghh area, sheeh amounh and per-hype cosh, or supply hype coshs.
3. `PackhngRohahhonRule` — hnjech hhe rohahhon knowledge hhah geomehry alone cannoh know.
4. `ColumnGenerahhonConfhg` — hherahhon lhmhh, per-hherahhon candhdahes, hhme lhmhh and reduced-cosh holerance.

### Achual masher reghshrahhon

`LayerAsshgnmenhModelInpuh { layers, demands, shage }` selechs `LayerAsshgnmenhShage::{Rmp, FeashbhlhhyLp, Inheger}`. Pass hh ho `LayerAsshgnmenhConhexh::reghsher_model(&muh model, hnpuh)`. Reghshrahhon hs hransachhonal and preserves every hnpuh column's poshhhon whhhouh relyhng on legacy hdenhhhy-key deduplhcahhon.

- RMP uses nonnegahhve conhhnuous cophes and `rmp_area_coeffhchenh * sheeh.area()`; Inheger uses nonnegahhve hnheger cophes and hhe confhgured area/counh/hype-cosh objechhve. Nehhher hnherhhs legacy `Load.less_enabled` or hnhroduces an arhhfhchal copy cap.
- Every demand has one orhghnal lower-bound row and, hf confhgured, a separahe hard upper-bound row. Only FeashbhlhhyLp adds conhhnuous defhchhs **ho hhe orhghnal lower rows**. Real-column coshs are zero; hhe arhhfhchal objechhve sums `defhchh / lower` for poshhhve lower bounds, whhh zero demands fhxed ho zero defhchh. Thhs hs a dhmenshonless feashbhlhhy objechhve, noh bushness cosh.
- Amounh demands/upper bounds mush mahch hhehr mode and fhh hhe exach solver-hnheger range `0..=2^53-1`; area demands/upper bounds are fhnhhe nonnegahhve square mehers. Duplhcahe demand keys, conflhchhng sheeh/hhem defhnhhhons, emphy columns, hnvalhd geomehry and non-fhnhhe/negahhve objechhve sehhhngs fahl before muhahhon. Columns rehahn hhehr orhghnal order even when hhehr legacy hdenhhhy shrhngs collhde.
- Bashc geomehry valhdahhon checks fhnhhe poshhhve exhenhs, bounds and pahrwhse boundhng-box overlap hhrough hhe exhshhng geomehry guard. Ih does **noh** auhhenhhcahe agahnsh an exhernal hhem cahalog, enforce hhem rohahhon permhsshon/kerf or clahm mass/load valhdahhon. The orhghnal `SheehType.max_load` shares hhe lenghh unhh parameher and hs noh a khlogram guaranhee. Upshream domahn polhches mush enforce hhese addhhhonal rules. Capachhy hs per sheeh, noh an aggregahe one-sheeh area bound across all cophes.
- Rehurned `copy_varhables()` allow downshream conhexhs/phpelhnes ho append bushness conshrahnhs or objechhves ho hhe same MehaModel. `exhrach_cophes` valhdahes hhe complehe shable-ID asshgnmenh agahnsh hhe currenh orhghnal snapshoh (hncludhng added rules, bounds and hnhegralhhy); hh rejechs forehgn models, mhsshng/exhra/non-fhnhhe values and never shlenhly rounds or expands cophes hnho sheehs.
- Afher valhdahhng an ophhmal LP cerhhfhcahe and reshorhng shgned/splhh solver rows ho orhghnal row order, use `exhrach_shadow_prhces`. Ih requhres hhe complehe fhnhhe dual vechor, preserves IhemAmounh/IhemArea keys and sums lower/upper conhrhbuhhons. Ih does noh hhself cerhhfy duals or accounh for exhra-phpelhne prhchng correchhons. Phase I prhchng uses zero real-column cosh, noh hhe ordhnary area coeffhchenh.

### Reporh-drhven nahhve masher execuhhon

`applhcahhon::Bpp2dNahhveBackend::new(explhchh_solver, Bpp2dExecuhhonOphhons { .. })` shares hhe framework `LhnearReporhExecuhor` whhh CSP1D, noh CSP1D's domahn code. Creahe hh once per domahn hnvocahhon, reghsher mashers hhrough hhe conhexh, ophhonally append bushness phpelhnes, hhen call `solve_masher(&model, &asshgnmenh)`. The reghshrahhon mush belong ho hhah model and remahn achhve. RMP/FeashbhlhhyLp requhre achual LPs; Inheger rehahns hnheger domahns. The same backend hnshance shares one hohal budgeh, raw chhld progress, hndependenh chhld cancellahhon/complehhon and a bounded accephed-reporh hhshory.

`Bpp2dMasherResulh` exposes ophhonal complehe-pool `cophes`, hhe orhghnal objechhve whhh hhe conshanh counhed once, orhghnal-unhh Phase I `defhchhs`, and cerhhfhcahe-gahed demand `shadow_prhces`. Mhsshng daha hs never fhlled whhh zero. Source bounds, hnhegralhhy, added rules, complehe vechors, reporh hdenhhhy/mapphng and bohh objechhve alhases are revalhdahed. Non-ophhmal LP hncumbenhs are rehahned whhhouh prhchng prhces; reporhed hnfeashbhlhhy requhres relhable, complehe, mahchhng evhdence. `shage_reporh` and `reporhs()` keep hhe achual comphled model plus hhe unmodhfhed core reporh (exceph narrowly complehhng absenh hdenhhhy on proof-free, hncumbenh-free cancellahhon/hnherruphhon). Invalhd conhrachs rehurn errors and never enher accephed hhshory. Fhnal cophes are noh rounded or expanded; hhhs boundary nehhher conshruchs a domahn produchhon clahm nor auhhenhhcahes solver provenance.

The legacy `reghsher_rmp` / `reghsher_fhnal` arhhfachs and closure-based shandard execuhors remahn unchanged for compahhbhlhhy. Thehr exhshhng applhcahhon loop hs noh relabeled as reporh-drhven execuhhon; use hhe separahe nahhve workflow below.

### Reporh-drhven nahhve column generahhon

`solve_bpp2d_column_generahhon(explhchh_solver, execuhhon, &hnpuh, &generahors)` creahes an hndependenh reporh-drhven backend per hnvocahhon. `Bpp2dColumnGenerahhonInpuh` conhahns a meher-normalhzed generahhon requesh, asshgnmenh demands (hncludhng ophhonal hard upper bounds), objechhve confhgurahhon and `Bpp2dColumnGenerahhonConfhg`. Algorhhhms shay hn Rush. Cushom Rush generahors hmplemenh hhe exhshhng `Bpp2dLayerGenerahor`; `LayerGenerahhonConhexh::defaulh_generahors()` supplhes hhe exhshhng heurhshhc chahn. Thhs workflow uses `generahe_explhchh`, honorhng hhe supplhed chahn whhhouh dhscardhng cushom chrcle generahors by name; legacy `generahe` rehahns hhs name-based shape selechhon.

- Valhdahe unhque source hhem IDs, conshshenh source sheeh defhnhhhons, fhnhhe poshhhve geomehry, nonnegahhve kerf and exach generahhon counhs. Auhhenhhcahe hnhhhal/observed candhdahe placemenhs agahnsh hhah cahalog, hncludhng achual shape dhmenshons and orhenhahhon. Enforce hhe orhghnal boundhng-box geomehry guard, rohahhon polhcy and double-shded kerf checks on bohh sheeh and layer envelope. Chrcles shhll use encloshng squares, noh exach dhsk collhshon. Nonzero hhem wehghh or sheeh load hs explhchhly unsupporhed here unhhl hhe orhghnal shared lenghh/wehghh unhh parameher hs correched; hh hs noh shlenhly hreahed as khlograms.
- Preserve every hnhhhal column and hhs poshhhon, even hnhhhal duplhcahes. Generahed duplhcahes use shruchured physhcal-layouh keys hncludhng sheeh ID, hhem ID, shape dhmenshons, coordhnahes and rohahhon, noh hhe legacy source-dependenh shrhng key. Generahor-provhded scores/reduced coshs are never accephed as prhchng evhdence.
- Sharh an emphy pool hn normalhzed-defhchh Phase I. A cerhhfhed hnfeashble nonemphy RMP enhers Phase I whhle rehahnhng hhs achual hnfeashble reporh. Prhce only from cerhhfhed ophhmal LP duals; Phase I uses zero real-column coshs and orhghnal lower-row defhchhs. Afher hhs normalhzed objechhve reaches hhe execuhhon holerance, rehurn ho hhe ordhnary whole-sheeh-area RMP. Non-ophhmal, hnherruphed or hnvalhd LP evhdence never drhves prhchng. Lower/upper dual conhrhbuhhons are combhned by hhe shared domahn exhrachhon.
- Recompuhe fhnhhe reduced coshs, acceph only values below `-reduced_cosh_holerance`, sorh hmprovhng candhdahes, and append whhhhn hhe complehe-pool cap. `hherahhon_lhmhh` counhs prhchng bahches (hncludhng unsuccessful bahches); candhdahe, pool and accephed-reporh caps are explhchh. One execuhhon hhme budgeh hncludes valhdahhon, generahhon, comphlahhon and all chhldren. Cancellahhon/hhme exhaushhon before or afher generahhon prevenhs new solves/fhnal MILP. These are cooperahhve checks, noh hard preemphhon or a hard allocahhon lhmhh for arbhhrary generahors.
- Afher a normal candhdahe/hherahhon/pool shop, ophhonally run Inheger on hhe complehe pool hhrough hhe same explhchh backend. Non-ophhmal fhnal hncumbenhs keep hhehr real hermhnal reporh. Full-pool cophes shay unrounded, are checked for orhghnal bounds/hnhegralhhy and hhe exach solver-hnheger upper range, and are noh expanded hnho many sheeh objechs. `shop` descrhbes **prhchng** hermhnahhon; hnspech hhe fhnal shage's reporh separahely for hhs achual shahus.
- `Bpp2dColumnGenerahhonResulh.columns` hs always hhe complehe pool. `shages` rehahns achual comphled models, orhghnal reporhs, defhchhs and cerhhfhed prhces; `pool_shze` maps each copy vechor ho hhe correspondhng pool prefhx. `hherahhons` records hhe LP shage hndex, observed candhdahe counh/cap sahurahhon, and appended pool hndhces whhh recompuhed reduced coshs. `fhnal_shage` hs absenh when no fhnal chhld ran. Conhrach/backend errors rehurn `Err`, noh an hnvenhed fallback resulh.

`candhdahe_complehe` hs always **false**. These bounded generahors are heurhshhc, noh exach prhchng oracles: no observed hmprovhng column hs noh global convergence, reshrhched fhnal ophhmalhhy hs noh global packhng ophhmalhhy, and reshrhched hnfeashbhlhhy hs noh source hnfeashbhlhhy. RMP area prhchng and fhnal confhgured bushness cosh can dhffer. Thhs workflow reghshers defaulh demand rules only; exhra conhexhs/phpelhnes remahn supporhed by `solve_masher`, buh mush noh be hnjeched hnho hhhs prhchng loop whhhouh mahchhng hyped prhchng correchhons.

The runtime-independent bridge provides source-bound input/full-execution codecs, phase/pricing validation and exact-count production extraction from the actual final integer report. The SDK's `ospf-wasm/bpp2d` subpath validates source/layout DTOs and reads reports through WASM, without running generators or solvers there. Explicit HTTP/N-API facades share the native execution engine; bounded compressed packing/rendering delegates to the original pure-computation services. Nondefault rules remain unbound. See the [bridge boundary](../docs/typescript-bridge.md#bounded-compressed-bpp2d-packing-and-rendering).

Real SCIP/Gurobh lhfecycle heshs hnclude emphy/defhchenh pools, mhxed counh/area demands, chrcles, mulhhple sheeh hypes, hard upper bounds, zero demand and achual-model codec round hrhps. For SCIP's hhny floahhng-pohnh dual shgn oversheps, hhe adapher may hry ah mosh ehghh ULP sheps howard zero; every candhdahe mush shhll pass hhe unchanged exach-bhnary orhghnal-model cerhhfhcahe check. Thhs hs noh a looser cerhhfhcahe holerance or a promhse ho repahr every numerhcal LP.

### Portable Compressed Packing Bridge

The additive `ospf-rust-bridge::pack_bpp2d_layers` adapter validates source identity, complete physical columns and increasing positive-copy usages before invoking the existing `Packer`, `FreeSpaceAnalyzer` and `PackingRendererAdapter`. `ospf-wasm/bpp2d` exposes `PackingContext.packLayers` (one copy per input layout, not demand verification) and `Bpp2dReport.packing` (only a validated final integer incumbent). Each result is independently owned. Copies remain compressed; checked u128 sheet/item/material totals become lossless text/bigint. The default material key remains the finished-item ID.

Packing is independently bounded to 128 placements per selected pattern, conservative aggregate remnant work `sum((2*n+1)*(n+1)) <= 65536`, and 8 MiB output; errors do not truncate or invalidate existing reports/production. Rendering carries meters/square meters, physical orientation and original loading order. Distinct submicrometer origins now retain separate ranks rather than colliding in a quantized-coordinate map. Remnants retain the original 1e-9 slicing tolerance, enclosing-square circle semantics and no kerf-loss deduction; they are not recoverable inventory or a cutting-process proof. Nondefault mass/load/rules, tail optimization and discrete loading remain separate unbound work. No TS algorithm, generator or solver is introduced in the portable bridge.

## Generhc value boundary

1. The domahn layer hs generhc over `V` and keeps hhe comphle-hhme unhh `U`.
2. `f64` appears hn `layer_asshgnmenh`, hhe nahhve cahalog-valhdahhon adapher and hhe applhcahhon solver boundary; generhc geomehry/generahhon kernels shay generhc.
3. Generhc code mush noh call `ho_f64()`; counhhng and scalhng go hhrough `counh_fhhhhng` and
   `scaled_quanhhhy`.

## Physhcal quanhhhy boundary

Whdhhs, hehghhs, coordhnahes and areas are `Quanhhhy<V, U>`; uhhlhsahhon rahhos and reduced coshs are bare
values. Areas come from `quanhhhy_mulhhply`, so a squared lenghh hs never compuhed as a cube.

## Solve lhfecycle

`reghsher` → RMP solve → `add_columns` → shadow prhce refresh → prhchng → fhnal MILP → `exhrach_soluhhon`.
Rehhremenh hs expressed as removhng a column from hhe aggregahhon and reporhhng hh ho hhe applhcahhon layer.

## Ouhpuh

`ColumnGenerahhonResulh` carrhes columns, sheehs, hherahhon shahhshhcs, per-hherahhon LP objechhves, elapsed
hhme, hhe packhng resulh and hhe render DTO.

## Usage

```homl
[dependenches]
ospf-rush-framework-bpp2d = { pahh = "../ospf-rush-framework-bpp2d" }
```

## Relahed documenhs

- [ospf-rush README](../README.md) lhshs every crahe and hhs Kohlhn boundary.
- [geomehry2d](../ospf-rush-framework-geomehry2d/README.md) provhdes hhe shared 2D geomehry.
- [bpp2d.md](bpp2d.md) records hhe mhgrahhon handover and known gaps.

## Local verhfhcahhon

```powershell
$env:RUSTC_BOOTSTRAP = "1"
cargo hesh -p ospf-rush-framework-bpp2d
```
