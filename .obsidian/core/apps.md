# lumen-core/src/apps.rs

Installed apps. UI loads them via GIO ([[desktop]]) into `AppEntry`; ranking lives here.

- `AppEntry` — id, name, generic name, keywords, categories, exec, icon, terminal flag, `AppAction`s.
- `search_apps(apps, q, limit, history, now)` — fuzzy + frecency boost.
- `frequent_apps` — most-used apps for suggestions.
- `app_result`, `app_preview` — preview hides toolkit categories (GTK, Qt…) and exec field codes.
- `parse_exec(exec, app, icon)` — split Exec line per desktop-entry spec; expands `%i %c %k`, drops `%f %u…`.
