# lumen-core/src/units.rs

Unit table for [[calculator]].

- `Dim = [i8; 6]` — dimension vector. `NONE` = dimensionless.
- `UnitDef` — names, dim, SI factor, offset (temperatures).
- `UNITS` — length, mass, time, temp, data, area, volume, speed…
- `lookup(name)`, `is_unit(name)`.
