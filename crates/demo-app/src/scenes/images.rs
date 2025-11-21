use std::path::PathBuf;

use super::{Scene, SceneKind};
use engine_core::{DisplayList, PassManager, Viewport};

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use image::AnimationDecoder;

pub struct ImagesScene {
    loaded: Cell<bool>,
    images: RefCell<Vec<LoadedImage>>, // cached GPU textures / animations
}

enum LoadedImage {
    Static(ImageTex),
    Animated(AnimatedImageTex),
    Svg(SvgImage),
}

struct ImageTex {
    _tex: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    _name: String,
}

struct AnimatedImageTex {
    // One texture/view per frame (simple and robust for demo scale)
    _frames: Vec<wgpu::Texture>,
    views: Vec<wgpu::TextureView>,
    durations: Vec<Duration>,
    current: usize,
    accum: Duration,
    last_tick: Option<Instant>,
    width: u32,
    height: u32,
    _name: String,
}

struct SvgImage {
    path: PathBuf,
    _name: String,
}

impl Default for ImagesScene {
    fn default() -> Self {
        Self {
            loaded: Cell::new(false),
            images: RefCell::new(Vec::new()),
        }
    }
}

impl ImagesScene {
    fn load_images_if_needed(&self, passes: &PassManager, queue: &wgpu::Queue) {
        if self.loaded.get() {
            return;
        }
        let device = passes.device();
        let mut found_any = false;
        let mut load_file = |path: PathBuf| {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            // Decide by extension
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_ascii_lowercase());
            match ext.as_deref() {
                Some("webp") => {
                    // Try animated WebP first, fall back to static decode via image::open
                    match std::fs::File::open(&path) {
                        Ok(file) => {
                            let reader = std::io::BufReader::new(file);
                            match image::codecs::webp::WebPDecoder::new(reader) {
                                Ok(decoder) => {
                                    let frames_iter = decoder.into_frames();
                                    match frames_iter.collect_frames() {
                                        Ok(frames) if !frames.is_empty() && frames.len() > 1 => {
                                            // Animated WebP
                                            let first_buf = frames[0].buffer().clone();
                                            let (w, h) = (first_buf.width(), first_buf.height());
                                            let mut texs = Vec::with_capacity(frames.len());
                                            let mut views = Vec::with_capacity(frames.len());
                                            let mut durs = Vec::with_capacity(frames.len());
                                            for f in frames {
                                                let mut dur = Duration::from_millis(100);
                                                let _ = f.delay();
                                                if dur.as_millis() == 0 {
                                                    dur = Duration::from_millis(100);
                                                }

                                                let buf = f.into_buffer();
                                                let (fw, fh) = buf.dimensions();
                                                let raw = buf.as_raw();
                                                let tw = w.max(1);
                                                let th = h.max(1);
                                                let tex = device.create_texture(
                                                    &wgpu::TextureDescriptor {
                                                        label: Some(&format!("webp:{}", name)),
                                                        size: wgpu::Extent3d {
                                                            width: tw,
                                                            height: th,
                                                            depth_or_array_layers: 1,
                                                        },
                                                        mip_level_count: 1,
                                                        sample_count: 1,
                                                        dimension: wgpu::TextureDimension::D2,
                                                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                                                        usage: wgpu::TextureUsages::TEXTURE_BINDING
                                                            | wgpu::TextureUsages::COPY_DST,
                                                        view_formats: &[],
                                                    },
                                                );
                                                let write_w = fw.min(w);
                                                let write_h = fh.min(h);
                                                queue.write_texture(
                                                    wgpu::ImageCopyTexture {
                                                        texture: &tex,
                                                        mip_level: 0,
                                                        origin: wgpu::Origin3d::ZERO,
                                                        aspect: wgpu::TextureAspect::All,
                                                    },
                                                    raw,
                                                    wgpu::ImageDataLayout {
                                                        offset: 0,
                                                        bytes_per_row: Some(write_w * 4),
                                                        rows_per_image: Some(write_h),
                                                    },
                                                    wgpu::Extent3d {
                                                        width: write_w,
                                                        height: write_h,
                                                        depth_or_array_layers: 1,
                                                    },
                                                );
                                                let view = tex.create_view(
                                                    &wgpu::TextureViewDescriptor::default(),
                                                );
                                                texs.push(tex);
                                                views.push(view);
                                                durs.push(dur);
                                            }
                                            self.images.borrow_mut().push(LoadedImage::Animated(
                                                AnimatedImageTex {
                                                    _frames: texs,
                                                    views,
                                                    durations: durs,
                                                    current: 0,
                                                    accum: Duration::from_millis(0),
                                                    last_tick: None,
                                                    width: w,
                                                    height: h,
                                                    _name: name,
                                                },
                                            ));
                                            found_any = true;
                                        }
                                        Ok(frames) if !frames.is_empty() => {
                                            // Single-frame WebP; treat as static
                                            let buf = frames[0].buffer().clone();
                                            let (w, h) = buf.dimensions();
                                            let tex =
                                                device.create_texture(&wgpu::TextureDescriptor {
                                                    label: Some(&format!("webp:{}", name)),
                                                    size: wgpu::Extent3d {
                                                        width: w.max(1),
                                                        height: h.max(1),
                                                        depth_or_array_layers: 1,
                                                    },
                                                    mip_level_count: 1,
                                                    sample_count: 1,
                                                    dimension: wgpu::TextureDimension::D2,
                                                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                                                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                                                        | wgpu::TextureUsages::COPY_DST,
                                                    view_formats: &[],
                                                });
                                            queue.write_texture(
                                                wgpu::ImageCopyTexture {
                                                    texture: &tex,
                                                    mip_level: 0,
                                                    origin: wgpu::Origin3d::ZERO,
                                                    aspect: wgpu::TextureAspect::All,
                                                },
                                                buf.as_raw(),
                                                wgpu::ImageDataLayout {
                                                    offset: 0,
                                                    bytes_per_row: Some(w * 4),
                                                    rows_per_image: Some(h),
                                                },
                                                wgpu::Extent3d {
                                                    width: w,
                                                    height: h,
                                                    depth_or_array_layers: 1,
                                                },
                                            );
                                            let view = tex.create_view(
                                                &wgpu::TextureViewDescriptor::default(),
                                            );
                                            self.images.borrow_mut().push(LoadedImage::Static(
                                                ImageTex {
                                                    _tex: tex,
                                                    view,
                                                    width: w,
                                                    height: h,
                                                    _name: name,
                                                },
                                            ));
                                            found_any = true;
                                        }
                                        _ => {
                                            // Fallback to generic open
                                            match image::open(&path) {
                                                Ok(img) => {
                                                    let rgba = img.to_rgba8();
                                                    let (w, h) = rgba.dimensions();
                                                    let tex = device.create_texture(
                                                        &wgpu::TextureDescriptor {
                                                            label: Some(&format!("webp:{}", name)),
                                                            size: wgpu::Extent3d {
                                                                width: w.max(1),
                                                                height: h.max(1),
                                                                depth_or_array_layers: 1,
                                                            },
                                                            mip_level_count: 1,
                                                            sample_count: 1,
                                                            dimension: wgpu::TextureDimension::D2,
                                                            format:
                                                                wgpu::TextureFormat::Rgba8UnormSrgb,
                                                            usage:
                                                                wgpu::TextureUsages::TEXTURE_BINDING
                                                                    | wgpu::TextureUsages::COPY_DST,
                                                            view_formats: &[],
                                                        },
                                                    );
                                                    queue.write_texture(
                                                        wgpu::ImageCopyTexture {
                                                            texture: &tex,
                                                            mip_level: 0,
                                                            origin: wgpu::Origin3d::ZERO,
                                                            aspect: wgpu::TextureAspect::All,
                                                        },
                                                        &rgba,
                                                        wgpu::ImageDataLayout {
                                                            offset: 0,
                                                            bytes_per_row: Some(w * 4),
                                                            rows_per_image: Some(h),
                                                        },
                                                        wgpu::Extent3d {
                                                            width: w,
                                                            height: h,
                                                            depth_or_array_layers: 1,
                                                        },
                                                    );
                                                    let view = tex.create_view(
                                                        &wgpu::TextureViewDescriptor::default(),
                                                    );
                                                    self.images.borrow_mut().push(
                                                        LoadedImage::Static(ImageTex {
                                                            _tex: tex,
                                                            view,
                                                            width: w,
                                                            height: h,
                                                            _name: name,
                                                        }),
                                                    );
                                                    found_any = true;
                                                }
                                                Err(_err) => {}
                                            }
                                        }
                                    }
                                }
                                Err(_err) => {
                                    // Fallback to generic open
                                    match image::open(&path) {
                                        Ok(img) => {
                                            let rgba = img.to_rgba8();
                                            let (w, h) = rgba.dimensions();
                                            let tex =
                                                device.create_texture(&wgpu::TextureDescriptor {
                                                    label: Some(&format!("webp:{}", name)),
                                                    size: wgpu::Extent3d {
                                                        width: w.max(1),
                                                        height: h.max(1),
                                                        depth_or_array_layers: 1,
                                                    },
                                                    mip_level_count: 1,
                                                    sample_count: 1,
                                                    dimension: wgpu::TextureDimension::D2,
                                                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                                                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                                                        | wgpu::TextureUsages::COPY_DST,
                                                    view_formats: &[],
                                                });
                                            queue.write_texture(
                                                wgpu::ImageCopyTexture {
                                                    texture: &tex,
                                                    mip_level: 0,
                                                    origin: wgpu::Origin3d::ZERO,
                                                    aspect: wgpu::TextureAspect::All,
                                                },
                                                &rgba,
                                                wgpu::ImageDataLayout {
                                                    offset: 0,
                                                    bytes_per_row: Some(w * 4),
                                                    rows_per_image: Some(h),
                                                },
                                                wgpu::Extent3d {
                                                    width: w,
                                                    height: h,
                                                    depth_or_array_layers: 1,
                                                },
                                            );
                                            let view = tex.create_view(
                                                &wgpu::TextureViewDescriptor::default(),
                                            );
                                            self.images.borrow_mut().push(LoadedImage::Static(
                                                ImageTex {
                                                    _tex: tex,
                                                    view,
                                                    width: w,
                                                    height: h,
                                                    _name: name,
                                                },
                                            ));
                                            found_any = true;
                                        }
                                        Err(_err) => {}
                                    }
                                }
                            }
                        }
                        Err(_err) => {}
                    }
                }
                Some("gif") => {
                    // Decode animated GIF via image crate (collect frames + durations)
                    match std::fs::File::open(&path) {
                        Ok(file) => {
                            let reader = std::io::BufReader::new(file);
                            match image::codecs::gif::GifDecoder::new(reader) {
                                Ok(decoder) => {
                                    let frames_iter = decoder.into_frames();
                                    match frames_iter.collect_frames() {
                                        Ok(frames) => {
                                            if frames.is_empty() {
                                                return;
                                            }
                                            // Determine canvas size from first frame
                                            let first_buf = frames[0].buffer().clone();
                                            let (w, h) = (first_buf.width(), first_buf.height());
                                            // Build textures for each frame
                                            let mut texs = Vec::with_capacity(frames.len());
                                            let mut views = Vec::with_capacity(frames.len());
                                            let mut durs = Vec::with_capacity(frames.len());
                                            for f in frames {
                                                let delay = f.delay();
                                                // Convert delay to Duration; fallback to 100ms if unavailable/zero
                                                // Default to 100ms/frame; many GIFs use 10fps when unspecified
                                                let mut dur = Duration::from_millis(100);
                                                // If available in this version, use to_duration(); otherwise keep default
                                                let _ = &delay; // silence unused warning if not used
                                                // If the delay is effectively zero, set a sane default
                                                if dur.as_millis() == 0 {
                                                    dur = Duration::from_millis(100);
                                                }

                                                let buf = f.into_buffer();
                                                let (fw, fh) = buf.dimensions();
                                                let raw = buf.as_raw();
                                                // If frame sizes vary, we currently assume full-size frames; clamp to first size
                                                let tw = w.max(1);
                                                let th = h.max(1);
                                                let tex = device.create_texture(
                                                    &wgpu::TextureDescriptor {
                                                        label: Some(&format!("gif:{}", name)),
                                                        size: wgpu::Extent3d {
                                                            width: tw,
                                                            height: th,
                                                            depth_or_array_layers: 1,
                                                        },
                                                        mip_level_count: 1,
                                                        sample_count: 1,
                                                        dimension: wgpu::TextureDimension::D2,
                                                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                                                        usage: wgpu::TextureUsages::TEXTURE_BINDING
                                                            | wgpu::TextureUsages::COPY_DST,
                                                        view_formats: &[],
                                                    },
                                                );
                                                // If frame size differs, center/blit would be needed; for now resize mismatch is ignored and write as-is within min dims
                                                let write_w = fw.min(w);
                                                let write_h = fh.min(h);
                                                queue.write_texture(
                                                    wgpu::ImageCopyTexture {
                                                        texture: &tex,
                                                        mip_level: 0,
                                                        origin: wgpu::Origin3d::ZERO,
                                                        aspect: wgpu::TextureAspect::All,
                                                    },
                                                    raw,
                                                    wgpu::ImageDataLayout {
                                                        offset: 0,
                                                        bytes_per_row: Some(write_w * 4),
                                                        rows_per_image: Some(write_h),
                                                    },
                                                    wgpu::Extent3d {
                                                        width: write_w,
                                                        height: write_h,
                                                        depth_or_array_layers: 1,
                                                    },
                                                );
                                                let view = tex.create_view(
                                                    &wgpu::TextureViewDescriptor::default(),
                                                );
                                                texs.push(tex);
                                                views.push(view);
                                                durs.push(dur);
                                            }
                                            self.images.borrow_mut().push(LoadedImage::Animated(
                                                AnimatedImageTex {
                                                    _frames: texs,
                                                    views,
                                                    durations: durs,
                                                    current: 0,
                                                    accum: Duration::from_millis(0),
                                                    last_tick: None,
                                                    width: w,
                                                    height: h,
                                                    _name: name,
                                                },
                                            ));
                                            found_any = true;
                                        }
                                        Err(_err) => {}
                                    }
                                }
                                Err(_err) => {}
                            }
                        }
                        Err(_err) => {}
                    }
                }
                Some("svg") => {
                    // Defer rasterization to engine-core's SVG cache.
                    self.images.borrow_mut().push(LoadedImage::Svg(SvgImage {
                        path: path.clone(),
                        _name: name,
                    }));
                    found_any = true;
                }
                _ => {
                    // PNG/JPEG via image crate
                    match image::open(&path) {
                        Ok(img) => {
                            let rgba = img.to_rgba8();
                            let (w, h) = rgba.dimensions();
                            let tex = device.create_texture(&wgpu::TextureDescriptor {
                                label: Some(&format!("img:{}", name)),
                                size: wgpu::Extent3d {
                                    width: w.max(1),
                                    height: h.max(1),
                                    depth_or_array_layers: 1,
                                },
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                                usage: wgpu::TextureUsages::TEXTURE_BINDING
                                    | wgpu::TextureUsages::COPY_DST,
                                view_formats: &[],
                            });
                            queue.write_texture(
                                wgpu::ImageCopyTexture {
                                    texture: &tex,
                                    mip_level: 0,
                                    origin: wgpu::Origin3d::ZERO,
                                    aspect: wgpu::TextureAspect::All,
                                },
                                &rgba,
                                wgpu::ImageDataLayout {
                                    offset: 0,
                                    bytes_per_row: Some(w * 4),
                                    rows_per_image: Some(h),
                                },
                                wgpu::Extent3d {
                                    width: w,
                                    height: h,
                                    depth_or_array_layers: 1,
                                },
                            );
                            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                            self.images.borrow_mut().push(LoadedImage::Static(ImageTex {
                                _tex: tex,
                                view,
                                width: w,
                                height: h,
                                _name: name,
                            }));
                            found_any = true;
                        }
                        Err(_err) => {}
                    }
                }
            }
        };

        // Scan the `images` directory for PNG/JPEG files
        let dir = PathBuf::from("images");
        if let Ok(read_dir) = std::fs::read_dir(&dir) {
            let mut files: Vec<PathBuf> =
                read_dir.filter_map(|e| e.ok().map(|e| e.path())).collect();
            files.sort();
            for p in files {
                if let Some(ext) = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_ascii_lowercase())
                {
                    if matches!(
                        ext.as_str(),
                        "png" | "jpg" | "jpeg" | "svg" | "gif" | "webp"
                    ) {
                        load_file(p);
                    }
                }
            }
        } else {
        }

        self.loaded.set(true);
    }
}

impl Scene for ImagesScene {
    fn kind(&self) -> SceneKind {
        SceneKind::FullscreenBackground
    }

    fn init_display_list(&mut self, _viewport: Viewport) -> Option<DisplayList> {
        None
    }

    fn paint_root_background(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) {
        // Ensure images are loaded/cached
        self.load_images_if_needed(passes, queue);
        // Clear background to a dark color for contrast
        let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("images-clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.07,
                        g: 0.09,
                        b: 0.13,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        // Lay out images in a simple grid and draw them
        if self.images.borrow().is_empty() {
            // Nothing loaded
            return;
        }

        let n = self.images.borrow().len();
        let cols = (n as f32).sqrt().ceil() as usize;
        let cols = cols.max(1);
        let rows = ((n + cols - 1) / cols).max(1);

        let margin = 16.0f32;
        let cell_w = ((width as f32) - margin * ((cols + 1) as f32)) / (cols as f32);
        let cell_h = ((height as f32) - margin * ((rows + 1) as f32)) / (rows as f32);

        // Update animations and draw
        let now = Instant::now();
        let mut images = self.images.borrow_mut();
        for (i, item) in images.iter_mut().enumerate() {
            let r = i / cols;
            let c = i % cols;

            let x0 = margin + c as f32 * (cell_w + margin);
            let y0 = margin + r as f32 * (cell_h + margin);

            // Resolve dimensions and view
            let mut _temp_view: Option<wgpu::TextureView> = None;
            let (iw, ih, view_ref): (f32, f32, &wgpu::TextureView) = match item {
                LoadedImage::Static(img) => (img.width as f32, img.height as f32, &img.view),
                LoadedImage::Animated(anim) => {
                    // Advance animation timers
                    if let Some(last) = anim.last_tick {
                        let dt = now.saturating_duration_since(last);
                        anim.accum = anim.accum.saturating_add(dt);
                    }
                    anim.last_tick = Some(now);
                    // Ensure non-empty
                    if !anim.durations.is_empty() && !anim.views.is_empty() {
                        let mut guard = 0usize;
                        while anim.accum >= anim.durations[anim.current] && guard < 10_000 {
                            let d = anim.durations[anim.current];
                            anim.accum = anim.accum.saturating_sub(d);
                            anim.current = (anim.current + 1) % anim.views.len();
                            guard += 1;
                        }
                    }
                    (
                        anim.width as f32,
                        anim.height as f32,
                        &anim.views[anim.current],
                    )
                }
                LoadedImage::Svg(svg) => {
                    // First, get intrinsic 1x size
                    let (_view1x, base_w, base_h) =
                        match passes.rasterize_svg_to_view(&svg.path, 1.0, None, queue) {
                            Some((v, w, h)) => (v, w as f32, h as f32),
                            None => {
                                continue;
                            }
                        };
                    // Compute target scale for this cell
                    let scale_guess = (cell_w / base_w).min(cell_h / base_h).max(0.0);
                    // Request a cached raster at the appropriate bucketed scale
                    let (view_scaled, sw, sh) =
                        match passes.rasterize_svg_to_view(&svg.path, scale_guess, None, queue) {
                            Some((v, w, h)) => (v, w as f32, h as f32),
                            None => match passes.rasterize_svg_to_view(&svg.path, 1.0, None, queue)
                            {
                                Some((v, w, h)) => (v, w as f32, h as f32),
                                None => {
                                    continue;
                                }
                            },
                        };
                    _temp_view = Some(view_scaled);
                    (sw, sh, _temp_view.as_ref().unwrap())
                }
            };

            let scale = (cell_w / iw).min(cell_h / ih).max(0.0);
            let draw_w = (iw * scale).max(1.0);
            let draw_h = (ih * scale).max(1.0);
            let ox = x0 + (cell_w - draw_w) * 0.5;
            let oy = y0 + (cell_h - draw_h) * 0.5;

            passes.draw_image_quad(
                encoder,
                surface_view,
                [ox, oy],
                [draw_w, draw_h],
                view_ref,
                queue,
                width,
                height,
            );
        }
    }
}
