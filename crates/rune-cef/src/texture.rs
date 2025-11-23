//! wgpu texture integration for CEF frame buffers.

use crate::error::{CefError, Result};
use crate::frame::{DirtyRect, FrameBuffer, PixelFormat};

/// Target for uploading CEF frames to wgpu textures.
/// Uses BGRA format to avoid CPU-side pixel conversion.
pub struct WgpuTextureTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    /// Staging buffer for efficient CPU->GPU transfer
    staging_buffer: Option<wgpu::Buffer>,
    staging_buffer_size: usize,
}

impl WgpuTextureTarget {
    /// Create a new texture target with the given dimensions.
    /// Uses BGRA format to match CEF's native output and avoid CPU conversion.
    pub fn new(device: &wgpu::Device, width: u32, height: u32, label: Option<&str>) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Use BGRA to match CEF's native format - no CPU conversion needed
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Pre-allocate staging buffer for full frame
        let buffer_size = (width * height * 4) as usize;
        let staging_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cef-staging-buffer"),
            size: buffer_size as u64,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: false,
        }));

        Self {
            texture,
            view,
            width,
            height,
            staging_buffer,
            staging_buffer_size: buffer_size,
        }
    }

    /// Get the texture view for binding.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the underlying texture.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// Get the current dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Resize the texture target. Creates a new texture if dimensions changed.
    /// Reuses existing texture if dimensions match.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        // Only recreate if dimensions actually changed
        self.texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cef-texture-resized"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.width = width;
        self.height = height;

        // Resize staging buffer if needed
        let new_size = (width * height * 4) as usize;
        if new_size > self.staging_buffer_size {
            self.staging_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cef-staging-buffer-resized"),
                size: new_size as u64,
                usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
                mapped_at_creation: false,
            }));
            self.staging_buffer_size = new_size;
        }
    }

    /// Upload a frame buffer to the texture.
    /// Supports dirty rect partial uploads for efficiency.
    /// Uses BGRA format directly - no CPU conversion needed.
    pub fn upload(&self, queue: &wgpu::Queue, frame: &FrameBuffer) -> Result<()> {
        if frame.width != self.width || frame.height != self.height {
            return Err(CefError::InvalidState(format!(
                "frame dimensions {}x{} don't match texture {}x{}",
                frame.width, frame.height, self.width, self.height
            )));
        }

        // CEF provides BGRA, our texture is BGRA - upload directly!
        // For RGBA input, we'd need conversion but CEF always outputs BGRA.
        let data = match frame.format {
            PixelFormat::Bgra8 => frame.as_bytes(),
            PixelFormat::Rgba8 => {
                // Rare case: if input is already RGBA, convert to BGRA for our texture
                let bgra = frame.as_bgra();
                return self.upload_bgra_data(queue, &bgra, &frame.get_dirty_rects(), frame.stride);
            }
        };

        self.upload_bgra_data(queue, data, &frame.get_dirty_rects(), frame.stride)
    }

    /// Upload BGRA data with dirty rect support.
    fn upload_bgra_data(
        &self,
        queue: &wgpu::Queue,
        data: &[u8],
        dirty_rects: &[DirtyRect],
        stride: u32,
    ) -> Result<()> {
        // For partial updates, upload only the dirty regions
        for rect in dirty_rects {
            if !rect.is_valid() {
                continue;
            }

            // Clamp rect to texture bounds
            let x = rect.x.min(self.width);
            let y = rect.y.min(self.height);
            let w = (rect.width).min(self.width - x);
            let h = (rect.height).min(self.height - y);

            if w == 0 || h == 0 {
                continue;
            }

            // For partial updates, we need to extract the rect data row-by-row
            // because the source stride may differ from rect width
            if w == self.width && h == self.height && x == 0 && y == 0 {
                // Full frame - direct upload
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &self.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(self.height),
                    },
                    wgpu::Extent3d {
                        width: self.width,
                        height: self.height,
                        depth_or_array_layers: 1,
                    },
                );
            } else {
                // Partial update - upload just the dirty rect
                // Calculate source offset
                let src_offset = (y * stride + x * 4) as usize;

                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &self.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x, y, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &data[src_offset..],
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(h),
                    },
                    wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }

        Ok(())
    }

    /// Upload full frame using staging buffer (for async uploads).
    #[allow(dead_code)]
    pub fn upload_staged(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &FrameBuffer,
    ) -> Result<()> {
        if frame.width != self.width || frame.height != self.height {
            return Err(CefError::InvalidState(format!(
                "frame dimensions {}x{} don't match texture {}x{}",
                frame.width, frame.height, self.width, self.height
            )));
        }

        let Some(ref staging) = self.staging_buffer else {
            // Fall back to direct upload
            return self.upload(queue, frame);
        };

        let data = frame.as_bytes();
        if data.len() > self.staging_buffer_size {
            return self.upload(queue, frame);
        }

        // Write to staging buffer
        queue.write_buffer(staging, 0, data);

        // Copy from staging to texture
        encoder.copy_buffer_to_texture(
            wgpu::ImageCopyBuffer {
                buffer: staging,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.width * 4),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    /// Create a bind group layout for sampling this texture.
    pub fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cef_texture_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    /// Create a bind group for sampling this texture.
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("cef_texture_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cef_texture_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        })
    }
}
