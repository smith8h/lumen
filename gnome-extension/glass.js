// Real frosted glass behind Lumen's panel.
//
// GNOME's own Shell.BlurEffect can blur what is behind an actor, but only as a plain
// rectangle, and it can't be wrapped in a mask effect (the mask's offscreen buffer would
// hide the background from it). So this effect builds the same paint-node pipeline
// itself — copy the pixels behind the actor, downscale, gaussian-blur — and then draws
// the result through a small shader that clips it to an anti-aliased rounded rectangle
// and adds a little saturation, like macOS vibrancy. Everything runs on the GPU; the
// JavaScript only assembles a handful of paint nodes per frame.

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Mtk from 'gi://Mtk';

/** Blur strength (gaussian sigma, in logical pixels). Strong enough that text behind the panel turns to soft colour. */
const SIGMA = 56;
/** Extra saturation, like `backdrop-filter: saturate()`. 1 = unchanged. */
const SATURATION = 1.6;
/**
 * The actor reaches this far past the panel on every side, so the blur samples real
 * pixels at the panel's edges instead of smearing its own border.
 */
export const MARGIN = 96;

// Firefox/GNOME-style downscaling: blur a smaller copy when the radius is large.
const MAX_SIGMA = 6;
const MIN_DOWNSCALE_SIZE = 256;

const MASK_DECLARATIONS = `
uniform vec4 u_box;     // the drawn part of the actor, in actor coordinates (x1, y1, x2, y2)
uniform vec4 u_panel;   // the panel inside the actor (x, y, width, height)
uniform float u_radius;
uniform float u_opacity;
uniform float u_saturation;
`;

const MASK_CODE = `
vec2 p = mix(u_box.xy, u_box.zw, cogl_tex_coord0_in.xy) - u_panel.xy;
vec2 half_size = u_panel.zw * 0.5;
vec2 q = abs(p - half_size) - (half_size - vec2(u_radius));
float dist = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - u_radius;
float coverage = clamp(0.5 - dist, 0.0, 1.0);

vec3 rgb = cogl_color_out.rgb;
float luma = dot(rgb, vec3(0.2126, 0.7152, 0.0722));
rgb = clamp(mix(vec3(luma), rgb, u_saturation), 0.0, 1.0);
cogl_color_out = vec4(rgb, 1.0) * (coverage * u_opacity);
`;

function downscaleFactor(width, height, sigma) {
    let factor = 1;
    while (sigma / factor > MAX_SIGMA && width / factor > MIN_DOWNSCALE_SIZE && height / factor > MIN_DOWNSCALE_SIZE)
        factor *= 2;
    return factor;
}

/** An offscreen framebuffer plus a pipeline that draws its texture. */
function makeTarget(ctx, width, height, withMask) {
    const texture = Cogl.Texture2D.new_with_size(ctx, width, height);
    const framebuffer = Cogl.Offscreen.new_with_texture(texture);
    framebuffer.allocate();
    framebuffer.orthographic(0, 0, width, height, 0, 1);
    const pipeline = Cogl.Pipeline.new(ctx);
    pipeline.set_layer_texture(0, texture);
    pipeline.set_layer_filters(0, Cogl.PipelineFilter.LINEAR, Cogl.PipelineFilter.LINEAR);
    pipeline.set_layer_wrap_mode(0, Cogl.PipelineWrapMode.CLAMP_TO_EDGE);
    if (withMask)
        pipeline.add_snippet(Cogl.Snippet.new(Cogl.SnippetHook.FRAGMENT, MASK_DECLARATIONS, MASK_CODE));
    return {width, height, texture, framebuffer, pipeline};
}

const RoundedBlurEffect = GObject.registerClass(
class LumenRoundedBlurEffect extends Clutter.Effect {
    _init() {
        super._init();
        this._background = null;
        this._result = null;
        this._cacheKey = null;
        this._redrawId = 0;
        this.panel = {x: 0, y: 0, width: 0, height: 0, radius: 0};
    }

    _target(name, ctx, width, height) {
        const current = this[name];
        if (current && current.width === width && current.height === height)
            return current;
        return (this[name] = makeTarget(ctx, width, height, name === '_result'));
    }

    /** Where this paint lands: the stage view (or offscreen, e.g. a screenshot) being drawn. */
    _viewFor(framebuffer) {
        for (const view of global.stage.peek_stage_views()) {
            if (view.get_framebuffer() === framebuffer)
                return {x: view.layout.x, y: view.layout.y, scale: view.get_scale()};
        }
        // Screenshots and screencasts paint the whole stage into one offscreen buffer.
        return {x: 0, y: 0, scale: framebuffer.get_width() / global.stage.width};
    }

    vfunc_paint_node(node, paintContext, _flags) {
        const actor = this.actor;
        const opacity = actor.get_paint_opacity() / 255;
        const {panel} = this;
        if (opacity === 0 || panel.width <= 0 || panel.height <= 0)
            return;

        const source = paintContext.get_framebuffer();
        const view = this._viewFor(source);
        const [actorWidth, actorHeight] = actor.get_size();
        const extents = actor.get_transformed_extents();
        const stageX = extents.origin.x, stageY = extents.origin.y;
        const stageW = extents.size.width, stageH = extents.size.height;
        if (actorWidth <= 0 || actorHeight <= 0 || stageW <= 0 || stageH <= 0)
            return;

        // The actor's rectangle in framebuffer pixels, clipped to the framebuffer.
        const fx1 = (stageX - view.x) * view.scale, fy1 = (stageY - view.y) * view.scale;
        const fx2 = fx1 + stageW * view.scale, fy2 = fy1 + stageH * view.scale;
        const cx1 = Math.max(0, Math.floor(fx1)), cy1 = Math.max(0, Math.floor(fy1));
        const cx2 = Math.min(source.get_width(), Math.ceil(fx2));
        const cy2 = Math.min(source.get_height(), Math.ceil(fy2));
        const width = cx2 - cx1, height = cy2 - cy1;
        if (width <= 0 || height <= 0)
            return;

        // The same rectangle in actor coordinates, which is what the node draws in.
        const box = new Clutter.ActorBox({
            x1: ((cx1 - fx1) / (fx2 - fx1)) * actorWidth,
            y1: ((cy1 - fy1) / (fy2 - fy1)) * actorHeight,
            x2: ((cx2 - fx1) / (fx2 - fx1)) * actorWidth,
            y2: ((cy2 - fy1) / (fy2 - fy1)) * actorHeight,
        });

        const ctx = source.get_context();
        const result = this._target('_result', ctx, width, height);
        const pipeline = result.pipeline;
        pipeline.set_uniform_float(pipeline.get_uniform_location('u_box'), 4, 1, [box.x1, box.y1, box.x2, box.y2]);
        pipeline.set_uniform_float(pipeline.get_uniform_location('u_panel'), 4, 1, [panel.x, panel.y, panel.width, panel.height]);
        pipeline.set_uniform_1f(pipeline.get_uniform_location('u_radius'), Math.min(panel.radius, panel.width / 2, panel.height / 2));
        pipeline.set_uniform_1f(pipeline.get_uniform_location('u_opacity'), opacity);
        pipeline.set_uniform_1f(pipeline.get_uniform_location('u_saturation'), SATURATION);

        // On a partial redraw, the framebuffer outside the redraw clip still holds the last
        // frame — including Lumen's own panel — so blurring it would smear old pixels into
        // the glass. Reuse the last blur for this frame and repaint the whole area next.
        const key = `${cx1},${cy1},${width},${height},${source.get_width()}x${source.get_height()}`;
        const clip = paintContext.get_redraw_clip();
        const rect = new Mtk.Rectangle({
            x: Math.floor(stageX), y: Math.floor(stageY),
            width: Math.ceil(stageW), height: Math.ceil(stageH),
        });
        const partial = clip && clip.contains_rectangle(rect) !== Mtk.RegionOverlap.IN;
        if (partial) {
            this._queueFullRedraw();
            if (this._cacheKey === key) {
                const cached = Clutter.PipelineNode.new(pipeline);
                node.add_child(cached);
                cached.add_rectangle(box);
                return;
            }
        }

        // result ← blur(downscaled copy of the pixels behind the actor)
        const scale = (stageW / actorWidth) * view.scale;
        const factor = downscaleFactor(width, height, SIGMA * scale);
        const smallW = Math.max(1, Math.round(width / factor));
        const smallH = Math.max(1, Math.round(height / factor));
        const background = this._target('_background', ctx, width, height);

        const resultNode = Clutter.LayerNode.new_to_framebuffer(result.framebuffer, pipeline);
        node.add_child(resultNode);
        resultNode.add_rectangle(box);

        const blurNode = Clutter.BlurNode.new(smallW, smallH, (SIGMA * scale) / factor);
        resultNode.add_child(blurNode);
        blurNode.add_rectangle(new Clutter.ActorBox({x1: 0, y1: 0, x2: width, y2: height}));

        const backgroundNode = Clutter.LayerNode.new_to_framebuffer(background.framebuffer, background.pipeline);
        blurNode.add_child(backgroundNode);
        backgroundNode.add_rectangle(new Clutter.ActorBox({x1: 0, y1: 0, x2: smallW, y2: smallH}));

        const blit = Clutter.BlitNode.new(source);
        backgroundNode.add_child(blit);
        blit.add_blit_rectangle(cx1, cy1, 0, 0, width, height);

        this._cacheKey = partial ? null : key;
    }

    _queueFullRedraw() {
        if (this._redrawId)
            return;
        this._redrawId = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._redrawId = 0;
            this.actor?.queue_redraw();
            return GLib.SOURCE_REMOVE;
        });
    }

    destroy() {
        if (this._redrawId)
            GLib.source_remove(this._redrawId);
        this._redrawId = 0;
        this._background = this._result = null;
    }
});

/**
 * An actor that shows blurred glass for a panel rectangle. Its own allocation is the panel
 * plus MARGIN on every side; the effect clips the glass to the panel's rounded corners.
 */
export const GlassActor = GObject.registerClass(
class LumenGlassActor extends Clutter.Actor {
    _init() {
        super._init({name: 'lumen-glass', reactive: false, opacity: 0, visible: false});
        this.set_pivot_point(0.5, 0.5);
        this._effect = new RoundedBlurEffect();
        this.add_effect_with_name('lumen-blur', this._effect);
        this._shown = false;
        this.connect('destroy', () => this._effect.destroy());
    }

    vfunc_get_paint_volume(volume) {
        return volume.set_from_allocation(this);
    }

    /** Places the glass under a panel at (x, y, width, height) in the parent's coordinates. */
    setPanel(x, y, width, height, radius) {
        if (width <= 0 || height <= 0) {
            this._hide();
            return;
        }
        this.set_position(x - MARGIN, y - MARGIN);
        this.set_size(width + 2 * MARGIN, height + 2 * MARGIN);
        this._effect.panel = {x: MARGIN, y: MARGIN, width, height, radius};
        this.queue_redraw();
        this._show();
    }

    // Follow the panel's own open/close animation in Lumen's CSS.
    _show() {
        if (this._shown)
            return;
        this._shown = true;
        this.remove_all_transitions();
        this.visible = true;
        this.set({opacity: 0, scale_x: 0.965, scale_y: 0.965, translation_y: -6});
        this.ease({opacity: 255, duration: 180, mode: Clutter.AnimationMode.EASE_OUT_CUBIC});
        this.ease({scale_x: 1, scale_y: 1, translation_y: 0, duration: 260, mode: Clutter.AnimationMode.EASE_OUT_BACK});
    }

    _hide() {
        if (!this._shown)
            return;
        this._shown = false;
        this.remove_all_transitions();
        this.ease({
            opacity: 0, scale_x: 0.965, scale_y: 0.965, translation_y: -6,
            duration: 120,
            mode: Clutter.AnimationMode.EASE_IN_CUBIC,
            onStopped: () => {
                if (!this._shown)
                    this.visible = false;
            },
        });
    }
});
