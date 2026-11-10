// Lumen companion for GNOME Shell.
//
// Lumen draws its panel inside a transparent full-screen Wayland window. GNOME doesn't
// let regular apps blur what's behind them, so this extension does it from inside the
// compositor:
//   • listens to Lumen's D-Bus signal PanelGeometry(x, y, w, h, radius),
//   • puts blurred glass under Lumen's window, clipped to the panel's rounded rectangle,
//   • keeps Lumen's window out of the dash, dock and Alt+Tab,
//   • makes sure the panel gets keyboard focus when it opens,
//   • sets Lumen's ShellBlur property so Lumen can make its own tint more see-through.
// Lumen works the same without it; the extension only adds to it.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {GlassActor} from './glass.js';

const BUS_NAME = 'io.github.lumenlauncher.Lumen';
const OBJECT_PATH = '/io/github/lumenlauncher/Lumen';
const INTERFACE = 'io.github.lumenlauncher.Lumen';
const WM_CLASSES = new Set(['lumen', 'Lumen']);

export default class LumenExtension extends Extension {
    enable() {
        this._tracked = new Map();
        this._geometry = null;
        this._lumenPid = 0;
        this._lumenOwner = null;
        this._signalId = 0;
        this._cancellable = new Gio.Cancellable();

        this._watchId = Gio.bus_watch_name(
            Gio.BusType.SESSION, BUS_NAME, Gio.BusNameWatcherFlags.NONE,
            (connection, _name, owner) => this._onLumenAppeared(connection, owner),
            () => this._onLumenVanished());

        this._windowCreatedId = global.display.connect('window-created', (_display, window) => this._track(window));
        this._trackExisting();
    }

    disable() {
        Gio.bus_unwatch_name(this._watchId);
        this._watchId = 0;
        global.display.disconnect(this._windowCreatedId);
        this._windowCreatedId = 0;
        this._cancellable.cancel();
        this._cancellable = null;

        if (this._lumenOwner) {
            Gio.DBus.session.signal_unsubscribe(this._signalId);
            // Tell Lumen the real blur is gone so it brings its own tint back.
            this._setShellBlur(false, null);
        }
        this._signalId = 0;
        this._lumenOwner = null;
        this._lumenPid = 0;

        for (const window of [...this._tracked.keys()])
            this._untrack(window, true);
        this._tracked = null;
        this._geometry = null;
    }

    // ── Lumen on D-Bus ───────────────────────────────────────────────────────

    _onLumenAppeared(connection, owner) {
        if (this._lumenOwner === owner)
            return;
        if (this._lumenOwner)
            connection.signal_unsubscribe(this._signalId);
        this._lumenOwner = owner;
        this._signalId = connection.signal_subscribe(
            owner, INTERFACE, 'PanelGeometry', OBJECT_PATH, null, Gio.DBusSignalFlags.NONE,
            (_conn, _sender, _path, _iface, _signal, params) => {
                const [x, y, width, height, radius] = params.deepUnpack();
                this._setGeometry({x, y, width, height, radius});
            });

        // The window belongs to the process that owns Lumen's bus name.
        connection.call(
            'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
            'GetConnectionUnixProcessID', new GLib.Variant('(s)', [owner]),
            new GLib.VariantType('(u)'), Gio.DBusCallFlags.NONE, -1, this._cancellable,
            (conn, res) => {
                try {
                    [this._lumenPid] = conn.call_finish(res).deepUnpack();
                    this._trackExisting();
                } catch (e) {
                    if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        console.warn(`Lumen: couldn't look up Lumen's process: ${e.message}`);
                }
            });

        // Lumen answers by re-sending the current panel geometry.
        this._setShellBlur(true, this._cancellable);
    }

    _onLumenVanished() {
        if (this._lumenOwner)
            Gio.DBus.session.signal_unsubscribe(this._signalId);
        this._signalId = 0;
        this._lumenOwner = null;
        this._lumenPid = 0;
        this._setGeometry(null);
    }

    _setShellBlur(enabled, cancellable) {
        Gio.DBus.session.call(
            BUS_NAME, OBJECT_PATH, 'org.freedesktop.DBus.Properties', 'Set',
            new GLib.Variant('(ssv)', [INTERFACE, 'ShellBlur', GLib.Variant.new_boolean(enabled)]),
            null, Gio.DBusCallFlags.NO_AUTO_START, 1000, cancellable,
            (conn, res) => {
                try {
                    conn.call_finish(res);
                } catch (e) {
                    // An older Lumen without the property, or Lumen just quit: nothing to do.
                    if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        console.debug(`Lumen: ShellBlur not set: ${e.message}`);
                }
            });
    }

    _setGeometry(geometry) {
        if (!this._tracked)
            return;
        this._geometry = geometry && geometry.width > 0 && geometry.height > 0 ? geometry : null;
        for (const entry of this._tracked.values())
            this._applyGeometry(entry);
    }

    _applyGeometry({glass}) {
        const g = this._geometry;
        if (!g)
            glass.setPanel(0, 0, 0, 0, 0);
        else
            glass.setPanel(g.x, g.y, g.width, g.height, g.radius);
    }

    // ── Lumen's window ───────────────────────────────────────────────────────

    _isLumen(window) {
        if (window.is_override_redirect())
            return false;
        if (this._lumenPid && window.get_pid() === this._lumenPid)
            return !window.get_title()?.startsWith('DevTools');
        return WM_CLASSES.has(window.get_wm_class()) && window.get_title() === 'Lumen';
    }

    _trackExisting() {
        for (const actor of global.get_window_actors())
            this._track(actor.meta_window);
    }

    _track(window) {
        if (!this._tracked || this._tracked.has(window) || !this._isLumen(window))
            return;
        const actor = window.get_compositor_private();
        if (!actor)
            return;

        // Keep the panel out of the dash, dock, Alt+Tab and the window list.
        window.hide_from_window_list();

        const glass = new GlassActor();
        actor.insert_child_at_index(glass, 0);
        const entry = {window, actor, glass, windowSignals: [], actorSignals: []};
        this._tracked.set(window, entry);

        entry.windowSignals.push(window.connect('unmanaged', () => this._untrack(window, false)));
        entry.windowSignals.push(window.connect('shown', () => this._focus(window)));
        entry.actorSignals.push(actor.connect('destroy', () => this._untrack(window, false, true)));
        entry.actorSignals.push(actor.connect('first-frame', () => this._focus(window)));

        this._applyGeometry(entry);
        if (window.showing_on_its_workspace?.())
            this._focus(window);
    }

    _focus(window) {
        if (window.has_focus())
            return;
        // Lumen asks for focus itself, but GNOME's focus-stealing prevention can turn that
        // into a "Lumen is ready" notification when the shortcut started it.
        Main.activateWindow(window, global.get_current_time());
    }

    _untrack(window, restore, actorGone = false) {
        const entry = this._tracked?.get(window);
        if (!entry)
            return;
        this._tracked.delete(window);
        for (const id of entry.windowSignals)
            window.disconnect(id);
        if (!actorGone) {
            for (const id of entry.actorSignals)
                entry.actor.disconnect(id);
            entry.glass.destroy();
        }
        if (restore)
            window.show_in_window_list();
    }
}
