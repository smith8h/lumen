//! Units for the calculator. Values are kept in SI base units; each unit has a dimension
//! vector, a factor (and an offset for temperatures).

/// Exponents of: length, mass, time, temperature, information (bits), angle.
pub type Dim = [i8; 6];

pub const NONE: Dim = [0; 6];
const L: Dim = [1, 0, 0, 0, 0, 0];
const M: Dim = [0, 1, 0, 0, 0, 0];
const T: Dim = [0, 0, 1, 0, 0, 0];
const K: Dim = [0, 0, 0, 1, 0, 0];
const D: Dim = [0, 0, 0, 0, 1, 0];
const A: Dim = [0, 0, 0, 0, 0, 1];
const AREA: Dim = [2, 0, 0, 0, 0, 0];
const VOLUME: Dim = [3, 0, 0, 0, 0, 0];
const SPEED: Dim = [1, 0, -1, 0, 0, 0];
const FREQ: Dim = [0, 0, -1, 0, 0, 0];
const ENERGY: Dim = [2, 1, -2, 0, 0, 0];
const POWER: Dim = [2, 1, -3, 0, 0, 0];
const PRESSURE: Dim = [-1, 1, -2, 0, 0, 0];
const FORCE: Dim = [1, 1, -2, 0, 0, 0];
const DATA_RATE: Dim = [0, 0, -1, 0, 1, 0];

pub struct UnitDef {
    /// Accepted spellings. The first one is how results are written.
    pub names: &'static [&'static str],
    /// Also accept the names in any letter case.
    pub any_case: bool,
    pub dim: Dim,
    pub factor: f64,
    pub offset: f64,
}

const fn u(names: &'static [&'static str], dim: Dim, factor: f64) -> UnitDef {
    UnitDef { names, any_case: true, dim, factor, offset: 0.0 }
}
const fn exact(names: &'static [&'static str], dim: Dim, factor: f64) -> UnitDef {
    UnitDef { names, any_case: false, dim, factor, offset: 0.0 }
}

const BYTE: f64 = 8.0;
const INCH: f64 = 0.0254;
const LB: f64 = 0.45359237;
const GAL: f64 = 3.785411784e-3;

pub static UNITS: &[UnitDef] = &[
    // Length
    u(&["m", "meter", "meters", "metre", "metres"], L, 1.0),
    u(&["km", "kilometer", "kilometers", "kilometre", "kilometres"], L, 1e3),
    u(&["cm", "centimeter", "centimeters", "centimetre", "centimetres"], L, 1e-2),
    u(&["mm", "millimeter", "millimeters", "millimetre", "millimetres"], L, 1e-3),
    u(&["µm", "um", "micrometer", "micrometers", "micron", "microns"], L, 1e-6),
    u(&["nm", "nanometer", "nanometers"], L, 1e-9),
    u(&["mi", "mile", "miles"], L, 1609.344),
    u(&["yd", "yard", "yards"], L, 0.9144),
    u(&["ft", "foot", "feet"], L, 0.3048),
    u(&["in", "inch", "inches"], L, INCH),
    u(&["nmi", "nautical mile", "nauticalmile", "nauticalmiles"], L, 1852.0),
    u(&["ly", "lightyear", "lightyears"], L, 9.4607304725808e15),
    u(&["au"], L, 1.495978707e11),
    // Mass
    u(&["kg", "kilogram", "kilograms", "kilo", "kilos"], M, 1.0),
    u(&["g", "gram", "grams", "gramme", "grammes"], M, 1e-3),
    u(&["mg", "milligram", "milligrams"], M, 1e-6),
    u(&["µg", "ug", "microgram", "micrograms"], M, 1e-9),
    u(&["t", "tonne", "tonnes", "ton", "tons"], M, 1e3),
    u(&["lb", "lbs", "pound", "pounds"], M, LB),
    u(&["oz", "ounce", "ounces"], M, LB / 16.0),
    u(&["st", "stone", "stones"], M, LB * 14.0),
    u(&["ct", "carat", "carats"], M, 2e-4),
    // Time
    u(&["s", "sec", "secs", "second", "seconds"], T, 1.0),
    u(&["ms", "millisecond", "milliseconds"], T, 1e-3),
    u(&["µs", "us", "microsecond", "microseconds"], T, 1e-6),
    u(&["ns", "nanosecond", "nanoseconds"], T, 1e-9),
    u(&["min", "mins", "minute", "minutes"], T, 60.0),
    u(&["h", "hr", "hrs", "hour", "hours"], T, 3600.0),
    u(&["day", "days", "d"], T, 86400.0),
    u(&["week", "weeks", "wk", "wks"], T, 604800.0),
    u(&["month", "months"], T, 2629800.0),
    u(&["year", "years", "yr", "yrs"], T, 31557600.0),
    u(&["decade", "decades"], T, 315576000.0),
    u(&["century", "centuries"], T, 3155760000.0),
    // Temperature (handled with offsets)
    UnitDef { names: &["°C", "degC", "celsius"], any_case: true, dim: K, factor: 1.0, offset: 273.15 },
    UnitDef { names: &["°F", "degF", "fahrenheit"], any_case: true, dim: K, factor: 5.0 / 9.0, offset: 273.15 - 32.0 * 5.0 / 9.0 },
    exact(&["K", "kelvin", "Kelvin"], K, 1.0),
    // Information. Lowercase "mb", "gb"… mean bytes; "Mb", "Gb"… are bits.
    exact(&["b", "bit", "bits"], D, 1.0),
    u(&["B", "byte", "bytes"], D, BYTE),
    u(&["KB", "kilobyte", "kilobytes"], D, BYTE * 1e3),
    u(&["MB", "megabyte", "megabytes"], D, BYTE * 1e6),
    u(&["GB", "gigabyte", "gigabytes"], D, BYTE * 1e9),
    u(&["TB", "terabyte", "terabytes"], D, BYTE * 1e12),
    u(&["PB", "petabyte", "petabytes"], D, BYTE * 1e15),
    u(&["KiB", "kibibyte", "kibibytes"], D, BYTE * 1024.0),
    u(&["MiB", "mebibyte", "mebibytes"], D, BYTE * 1048576.0),
    u(&["GiB", "gibibyte", "gibibytes"], D, BYTE * 1073741824.0),
    u(&["TiB", "tebibyte", "tebibytes"], D, BYTE * 1099511627776.0),
    exact(&["Kb", "kbit", "kilobit", "kilobits"], D, 1e3),
    exact(&["Mb", "Mbit", "megabit", "megabits"], D, 1e6),
    exact(&["Gb", "Gbit", "gigabit", "gigabits"], D, 1e9),
    exact(&["Tb", "Tbit", "terabit", "terabits"], D, 1e12),
    u(&["Mbps", "mbps"], DATA_RATE, 1e6),
    u(&["Gbps", "gbps"], DATA_RATE, 1e9),
    u(&["kbps"], DATA_RATE, 1e3),
    // Angle
    u(&["rad", "radian", "radians"], A, 1.0),
    u(&["deg", "degree", "degrees", "°"], A, std::f64::consts::PI / 180.0),
    u(&["grad", "gradian", "gradians"], A, std::f64::consts::PI / 200.0),
    u(&["turn", "turns", "rev", "revolution", "revolutions"], A, std::f64::consts::TAU),
    // Area
    u(&["m²", "sqm", "m2"], AREA, 1.0),
    u(&["km²", "sqkm", "km2"], AREA, 1e6),
    u(&["cm²", "sqcm", "cm2"], AREA, 1e-4),
    u(&["ft²", "sqft", "ft2"], AREA, 0.09290304),
    u(&["mi²", "sqmi", "mi2"], AREA, 2589988.110336),
    u(&["ha", "hectare", "hectares"], AREA, 1e4),
    u(&["acre", "acres", "ac"], AREA, 4046.8564224),
    // Volume
    u(&["L", "l", "liter", "liters", "litre", "litres"], VOLUME, 1e-3),
    u(&["mL", "ml", "milliliter", "milliliters", "millilitre", "millilitres", "cc"], VOLUME, 1e-6),
    u(&["m³", "m3", "cbm"], VOLUME, 1.0),
    u(&["cm³", "cm3"], VOLUME, 1e-6),
    u(&["gal", "gallon", "gallons"], VOLUME, GAL),
    u(&["qt", "quart", "quarts"], VOLUME, GAL / 4.0),
    u(&["pt", "pint", "pints"], VOLUME, GAL / 8.0),
    u(&["cup", "cups"], VOLUME, GAL / 16.0),
    u(&["floz", "fl oz"], VOLUME, GAL / 128.0),
    u(&["tbsp", "tablespoon", "tablespoons"], VOLUME, GAL / 256.0),
    u(&["tsp", "teaspoon", "teaspoons"], VOLUME, GAL / 768.0),
    // Speed
    u(&["km/h", "kph", "kmh"], SPEED, 1000.0 / 3600.0),
    u(&["mph"], SPEED, 1609.344 / 3600.0),
    u(&["kn", "knot", "knots", "kt"], SPEED, 1852.0 / 3600.0),
    // Frequency
    u(&["Hz", "hertz"], FREQ, 1.0),
    u(&["kHz", "kilohertz"], FREQ, 1e3),
    u(&["MHz", "megahertz"], FREQ, 1e6),
    u(&["GHz", "gigahertz"], FREQ, 1e9),
    // Energy
    u(&["J", "joule", "joules"], ENERGY, 1.0),
    u(&["kJ", "kilojoule", "kilojoules"], ENERGY, 1e3),
    u(&["cal", "calorie", "calories"], ENERGY, 4.184),
    u(&["kcal", "kilocalorie", "kilocalories"], ENERGY, 4184.0),
    u(&["Wh", "watthour", "watthours"], ENERGY, 3600.0),
    u(&["kWh", "kilowatthour", "kilowatthours"], ENERGY, 3.6e6),
    u(&["eV", "electronvolt", "electronvolts"], ENERGY, 1.602176634e-19),
    u(&["BTU", "btu"], ENERGY, 1055.05585262),
    // Power
    u(&["W", "watt", "watts"], POWER, 1.0),
    u(&["kW", "kilowatt", "kilowatts"], POWER, 1e3),
    u(&["MW", "megawatt", "megawatts"], POWER, 1e6),
    u(&["hp", "horsepower"], POWER, 745.699_871_582_270_2),
    // Pressure
    u(&["Pa", "pascal", "pascals"], PRESSURE, 1.0),
    u(&["kPa", "kilopascal", "kilopascals"], PRESSURE, 1e3),
    u(&["bar", "bars"], PRESSURE, 1e5),
    u(&["psi"], PRESSURE, 6894.757293168),
    u(&["atm", "atmosphere", "atmospheres"], PRESSURE, 101325.0),
    u(&["mmHg"], PRESSURE, 133.322387415),
    // Force
    u(&["N", "newton", "newtons"], FORCE, 1.0),
    u(&["kN", "kilonewton", "kilonewtons"], FORCE, 1e3),
    u(&["lbf"], FORCE, 4.4482216152605),
];

pub fn lookup(name: &str) -> Option<&'static UnitDef> {
    UNITS.iter().find(|u| u.names.contains(&name)).or_else(|| {
        let lower = name.to_lowercase();
        UNITS.iter().find(|u| u.any_case && u.names.iter().any(|n| n.to_lowercase() == lower))
    })
}

pub fn is_unit(name: &str) -> bool {
    lookup(name).is_some()
}
