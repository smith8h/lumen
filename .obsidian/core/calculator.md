# lumen-core/src/calculator.rs

Calculator plus unit and currency conversion. Biggest core module.

Examples: `2^10`, `sqrt(2)`, `15% of 80`, `100 + 10%`, `5 km to mi`, `30 c to f`, `5 GB to MB`, `255 to hex`, `100 usd to iqd`.

## Pipeline
1. `normalize` — tidy input (×, ÷, words).
2. `looks_like_math` — decide whether to try.
3. `tokenize` → `Tok`s.
4. Recursive descent `Parser`: additive → term → unary → power → postfix (`!`, `%`) → primary. Implicit multiplication. `to`/`in` targets (unit, hex, bin, oct…).
5. `Val` = number + optional `Unit` (dimension vector from [[units]]). Dimension-checked add/mul/div/pow.
6. `call` — functions (sqrt, sin, log, gcd, round…). `constant` — pi, e.
7. `format_val` — via [[format]].

## Public
- `evaluate(expr) -> Option<String>`
- `plain(s)` — strip grouping for copy.
- `calculate(query, currency_conversion, rates)` → result items. Currency via [[rates]].
