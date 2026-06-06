use std::path::PathBuf;
use std::sync::mpsc;

pub(crate) struct PendingPngReadback {
    buffer: wgpu::Buffer,
    path: PathBuf,
    width: u32,
    height: u32,
    unpadded_bytes_per_row: u32,
    padded_bytes_per_row: u32,
    format: wgpu::TextureFormat,
}

pub(crate) fn enqueue_png_readback(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    path: PathBuf,
) -> PendingPngReadback {
    let unpadded_bytes_per_row = width * 4;
    let padded_bytes_per_row =
        wgpu::util::align_to(unpadded_bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("map_phase0_readback"),
        size: (padded_bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    PendingPngReadback {
        buffer,
        path,
        width,
        height,
        unpadded_bytes_per_row,
        padded_bytes_per_row,
        format,
    }
}

pub(crate) fn finish_png_readback(
    device: &wgpu::Device,
    pending: PendingPngReadback,
) -> Result<(), String> {
    let swizzle = match pending.format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => [0, 1, 2, 3],
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => [2, 1, 0, 3],
        other => return Err(format!("unsupported screenshot format {other:?}")),
    };

    let buffer_slice = pending.buffer.slice(..);
    let (tx, rx) = mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result.map_err(|err| format!("{err:?}")));
    });
    let _ = device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .map_err(|err| format!("readback callback failed: {err}"))??;

    let mapped = buffer_slice.get_mapped_range();
    let mut rgba = vec![0u8; (pending.width * pending.height * 4) as usize];
    for y in 0..pending.height as usize {
        let src_start = y * pending.padded_bytes_per_row as usize;
        let src_end = src_start + pending.unpadded_bytes_per_row as usize;
        let src = &mapped[src_start..src_end];
        let dst = &mut rgba[y * pending.width as usize * 4..(y + 1) * pending.width as usize * 4];
        for (src_px, dst_px) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
            dst_px[0] = src_px[swizzle[0]];
            dst_px[1] = src_px[swizzle[1]];
            dst_px[2] = src_px[swizzle[2]];
            dst_px[3] = src_px[swizzle[3]];
        }
    }
    drop(mapped);
    pending.buffer.unmap();

    if let Some(parent) = pending.path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("create output dir: {err}"))?;
    }
    let file = std::fs::File::create(&pending.path).map_err(|err| format!("create png: {err}"))?;
    let writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, pending.width, pending.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|err| format!("png header: {err}"))?
        .write_image_data(&rgba)
        .map_err(|err| format!("png data: {err}"))?;
    Ok(())
}
