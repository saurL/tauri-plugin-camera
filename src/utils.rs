use yuv::{
    YuvBiPlanarImageMut, YuvChromaSubsampling, YuvConversionMode, YuvPlanarImageMut, YuvRange,
    YuvStandardMatrix,
};

/// Utility functions for image format conversion and processing
use crate::error::{Error, Result};

/// Convert YUV (I420/YV12) buffer to RGB24
///
/// # Arguments
/// * `yuv_data` - Input YUV buffer (planar format: Y plane, U plane, V plane)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
///
/// # Returns
/// RGB24 buffer where each pixel is 3 bytes (R, G, B)
pub fn yuv_to_rgb(yuv_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let width_usize = width as usize;
    let height_usize = height as usize;

    // Validate input buffer size for I420 format
    // I420 format: Y plane (width*height) + U plane (width*height/4) + V plane (width*height/4)
    let expected_size = width * height * 3 / 2;
    if yuv_data.len() < expected_size as usize {
        return Err(Error::CameraError(format!(
            "Invalid YUV buffer size: expected at least {}, got {}",
            expected_size,
            yuv_data.len()
        )));
    }

    // Allocate YUV planar image and copy input data into it
    let mut planar_image_mut =
        YuvPlanarImageMut::<u8>::alloc(width, height, YuvChromaSubsampling::Yuv420);

    // Copy Y plane (first width*height bytes)
    let y_plane_size = width_usize * height_usize;
    planar_image_mut.y_plane.copy_from_slice(&yuv_data[0..y_plane_size]);

    // Copy U plane (next width*height/4 bytes)
    let u_plane_size = width_usize * height_usize / 4;
    planar_image_mut
        .u_plane
        .copy_from_slice(&yuv_data[y_plane_size..y_plane_size + u_plane_size]);

    // Copy V plane (next width*height/4 bytes)
    let v_plane_offset = y_plane_size + u_plane_size;
    planar_image_mut
        .v_plane
        .copy_from_slice(&yuv_data[v_plane_offset..v_plane_offset + u_plane_size]);

    let planar_image = planar_image_mut.to_fixed();

    let rgb_data_size = (width * height * 3) as usize;
    let mut rgb_data = vec![0u8; rgb_data_size];
    let rgba_stride = width * 3;

    // Convert using yuv crate
    yuv::yuv420_to_rgb(
        &planar_image,
        &mut rgb_data,
        rgba_stride,
        YuvRange::Limited,
        YuvStandardMatrix::Bt601,
    )
    .map_err(|e| Error::CameraError(format!("YUV to RGB conversion failed: {:?}", e)))?;

    Ok(rgb_data)
}

/// Convert YUV (NV12) buffer to RGB24
///
/// # Arguments
/// * `yuv_data` - Input YUV buffer (NV12 format: Y plane, interleaved UV plane)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
///
/// # Returns
/// RGB24 buffer where each pixel is 3 bytes (R, G, B)
pub fn nv12_to_rgb(yuv_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let width_usize = width as usize;
    let height_usize = height as usize;

    // Validate input buffer size for NV12 format
    // NV12 format: Y plane (width*height) + UV plane (width*height/2)
    let expected_size = width_usize * height_usize * 3 / 2;
    if yuv_data.len() < expected_size {
        return Err(Error::CameraError(format!(
            "Invalid NV12 buffer size: expected at least {}, got {}",
            expected_size,
            yuv_data.len()
        )));
    }

    // Allocate output RGB buffer (3 bytes per pixel)
    let mut rgb_data = vec![0u8; width_usize * height_usize * 3];

    // Allocate YUV biplanar image and copy input data into it
    let mut planar_image_mut = YuvBiPlanarImageMut::alloc(width, height, YuvChromaSubsampling::Yuv420);

    // Copy Y plane (first width*height bytes)
    let y_plane_size = width_usize * height_usize;
    planar_image_mut.y_plane.copy_from_slice(&yuv_data[0..y_plane_size]);

    // Copy UV plane (next width*height/2 bytes)
    let uv_plane_size = width_usize * height_usize / 2;
    planar_image_mut.uv_plane.copy_from_slice(&yuv_data[y_plane_size..y_plane_size + uv_plane_size]);

    let planar_image = planar_image_mut.to_fixed();

    // Convert using yuv crate
    let rgba_stride = width * 3;
    yuv::yuv_nv12_to_rgb(
        &planar_image,
        &mut rgb_data,
        rgba_stride,
        YuvRange::Limited,
        YuvStandardMatrix::Bt601,
        YuvConversionMode::Balanced,
    )
    .map_err(|e| Error::CameraError(format!("NV12 to RGB conversion failed: {:?}", e)))?;

    Ok(rgb_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yuv_to_rgb_buffer_size() {
        let width = 640u32;
        let height = 480u32;
        let yuv_size = (width * height * 3 / 2) as usize;
        let yuv_data = vec![0u8; yuv_size];

        let result = yuv_to_rgb(&yuv_data, width, height);
        assert!(result.is_ok());

        let rgb_data = result.unwrap();
        assert_eq!(rgb_data.len(), (width * height * 3) as usize);
    }

    #[test]
    fn test_yuv_to_rgb_invalid_size() {
        let width = 640u32;
        let height = 480u32;
        let yuv_data = vec![0u8; 100]; // Too small

        let result = yuv_to_rgb(&yuv_data, width, height);
        assert!(result.is_err());
    }

    #[test]
    fn test_nv12_to_rgb_buffer_size() {
        let width = 640u32;
        let height = 480u32;
        let nv12_size = (width * height * 3 / 2) as usize;
        let nv12_data = vec![0u8; nv12_size];

        let result = nv12_to_rgb(&nv12_data, width, height);
        assert!(result.is_ok());

        let rgb_data = result.unwrap();
        assert_eq!(rgb_data.len(), (width * height * 3) as usize);
    }
}
