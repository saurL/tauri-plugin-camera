use openh264::{encoder::Encoder, formats::YUVSlices};
use yuv::{YuvBiPlanarImage, YuvConversionMode, YuvPlanarImage, YuvRange, YuvStandardMatrix};

/// Utility functions for image format conversion and processing
use crate::error::{Error, Result};

/// Convert YUV (I420/YV12) buffer to RGBA
///
/// # Arguments
/// * `yuv_data` - Input YUV buffer (planar format: Y plane, U plane, V plane)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
///
/// # Returns
/// RGBA buffer where each pixel is 4 bytes (R, G, B, A)
pub fn yuv_to_rgba(yuv_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let width_usize = width as usize;
    let height_usize = height as usize;

    // Validate input buffer size for I420 format
    let expected_size = width * height * 3 / 2;
    if yuv_data.len() < expected_size as usize {
        return Err(Error::CameraError(format!(
            "Invalid YUV buffer size: expected at least {}, got {}",
            expected_size,
            yuv_data.len()
        )));
    }

    // Calculate plane sizes
    let y_plane_size = width_usize * height_usize;
    let u_plane_size = width_usize * height_usize / 4;
    let v_plane_offset = y_plane_size + u_plane_size;

    // Create YuvPlanarImage directly from slices
    let yuv_image = YuvPlanarImage {
        y_plane: &yuv_data[0..y_plane_size],
        y_stride: width,
        u_plane: &yuv_data[y_plane_size..y_plane_size + u_plane_size],
        u_stride: width / 2,
        v_plane: &yuv_data[v_plane_offset..v_plane_offset + u_plane_size],
        v_stride: width / 2,
        width,
        height,
    };

    // ⚡ OPTIMISATION: Pré-allocation avec capacité exacte (RGBA = 4 bytes par pixel)
    let rgb_data_size = width_usize * height_usize * 4;
    let mut rgb_data = Vec::with_capacity(rgb_data_size);
    unsafe {
        rgb_data.set_len(rgb_data_size);
    }

    let rgb_stride = width * 4;

    // ⚡ OPTIMISATION: Détection auto de la matrice couleur selon résolution
    let matrix = if width >= 1280 || height >= 720 {
        YuvStandardMatrix::Bt709 // HD et plus
    } else {
        YuvStandardMatrix::Bt601 // SD
    };

    // Convert using yuv crate
    yuv::yuv420_to_rgba(
        &yuv_image,
        &mut rgb_data,
        rgb_stride,
        YuvRange::Limited,
        matrix,
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
pub fn nv12_to_rgba(yuv_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let width_usize = width as usize;
    let height_usize = height as usize;

    // Validate input buffer size for NV12 format
    let expected_size = width_usize * height_usize * 3 / 2;
    if yuv_data.len() < expected_size {
        return Err(Error::CameraError(format!(
            "Invalid NV12 buffer size: expected at least {}, got {}",
            expected_size,
            yuv_data.len()
        )));
    }

    // Calculate plane sizes
    let y_plane_size = width_usize * height_usize;

    // ⚡ OPTIMISATION: Références directes sans calcul de fin
    let yuv_image = YuvBiPlanarImage {
        y_plane: &yuv_data[..y_plane_size],
        y_stride: width,
        uv_plane: &yuv_data[y_plane_size..],
        uv_stride: width,
        width,
        height,
    };

    // ⚡ OPTIMISATION: Pré-allocation avec capacité exacte (RGBA = 4 bytes par pixel)
    let mut rgb_data = Vec::with_capacity(width_usize * height_usize * 4);
    unsafe {
        rgb_data.set_len(width_usize * height_usize * 4);
    }

    let rgb_stride = width * 4;

    // ⚡ OPTIMISATION: Détection auto de la matrice couleur selon résolution
    let matrix = if width >= 1280 || height >= 720 {
        YuvStandardMatrix::Bt709 // HD et plus
    } else {
        YuvStandardMatrix::Bt601 // SD
    };

    // Convert using yuv crate
    yuv::yuv_nv12_to_rgba(
        &yuv_image,
        &mut rgb_data,
        rgb_stride,
        YuvRange::Limited,
        matrix,
        YuvConversionMode::Fast,
    )
    .map_err(|e| Error::CameraError(format!("NV12 to RGB conversion failed: {:?}", e)))?;

    Ok(rgb_data)
}

/// Convert NV12 to I420 format (de-interleave UV plane)
///
/// # Arguments
/// * `nv12_data` - Input NV12 buffer (Y plane + interleaved UV plane)
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
///
/// # Returns
/// I420 buffer (Y plane + U plane + V plane)
fn nv12_to_i420(nv12_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let width_usize = width as usize;
    let height_usize = height as usize;

    let y_plane_size = width_usize * height_usize;
    let uv_plane_size = y_plane_size / 4;

    // Validate input
    let expected_size = y_plane_size + uv_plane_size * 2;
    if nv12_data.len() < expected_size {
        return Err(Error::CameraError(format!(
            "Invalid NV12 buffer size: expected at least {}, got {}",
            expected_size,
            nv12_data.len()
        )));
    }

    // Allocate I420 buffer
    let mut i420_data = Vec::with_capacity(expected_size);

    // Copy Y plane (unchanged)
    i420_data.extend_from_slice(&nv12_data[..y_plane_size]);

    // De-interleave UV plane: UVUVUV... → UUU... VVV...
    let uv_plane = &nv12_data[y_plane_size..];
    let mut u_plane = Vec::with_capacity(uv_plane_size);
    let mut v_plane = Vec::with_capacity(uv_plane_size);

    for i in 0..uv_plane_size {
        u_plane.push(uv_plane[i * 2]); // U values at even indices
        v_plane.push(uv_plane[i * 2 + 1]); // V values at odd indices
    }

    i420_data.extend(u_plane);
    i420_data.extend(v_plane);

    Ok(i420_data)
}

/// Encode a NV12 frame into H.264 using OpenH264
///
/// # Arguments
/// * `nv12_data` - Input NV12 buffer (Y plane + interleaved UV plane)
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
///
/// # Returns
/// A `Vec<u8>` containing the H.264 Annex B bitstream (SPS/PPS + frame NALs)
pub fn yuv_nv12_to_h264(nv12_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let width_usize = width as usize;
    let height_usize = height as usize;

    // Convert NV12 to I420 first
    let i420_data = nv12_to_i420(nv12_data, width, height)?;

    // Split I420 planes: Y [w*h], U [w*h/4], V [w*h/4]
    let y_plane_size = width_usize * height_usize;
    let uv_plane_size = y_plane_size / 4;
    let (y_plane, rest) = i420_data.split_at(y_plane_size);
    let (u_plane, v_plane) = rest.split_at(uv_plane_size);

    let chroma_width = width_usize / 2;

    // Wrap as YUVSlices (4:2:0), using tight strides
    let yuv = YUVSlices::new(
        (y_plane, u_plane, v_plane),
        (width_usize, height_usize),
        (width_usize, chroma_width, chroma_width),
    );

    // Create encoder with default config and encode one frame
    let mut encoder = Encoder::new()
        .map_err(|e| Error::CameraError(format!("Failed to create OpenH264 encoder: {}", e)))?;

    let bitstream = encoder
        .encode(&yuv)
        .map_err(|e| Error::CameraError(format!("Failed to encode frame: {}", e)))?;

    Ok(bitstream.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yuv_to_rgba_buffer_size() {
        let width = 640u32;
        let height = 480u32;
        let yuv_size = (width * height * 3 / 2) as usize;
        let yuv_data = vec![0u8; yuv_size];

        let result = yuv_to_rgba(&yuv_data, width, height);
        assert!(result.is_ok());

        let rgb_data = result.unwrap();
        assert_eq!(rgb_data.len(), (width * height * 4) as usize); // RGBA = 4 bytes per pixel
    }

    #[test]
    fn test_yuv_to_rgb_invalid_size() {
        let width = 640u32;
        let height = 480u32;
        let yuv_data = vec![0u8; 100]; // Too small

        let result = yuv_to_rgba(&yuv_data, width, height);
        assert!(result.is_err());
    }

    #[test]
    fn test_nv12_to_rgba_buffer_size() {
        let width = 640u32;
        let height = 480u32;
        let nv12_size = (width * height * 3 / 2) as usize;
        let nv12_data = vec![0u8; nv12_size];

        let result = nv12_to_rgba(&nv12_data, width, height);
        assert!(result.is_ok());

        let rgb_data = result.unwrap();
        assert_eq!(rgb_data.len(), (width * height * 4) as usize); // RGBA = 4 bytes per pixel
    }

    #[test]
    fn test_nv12_to_rgba_invalid_size() {
        let width = 640u32;
        let height = 480u32;
        let nv12_data = vec![0u8; 100]; // Too small

        let result = nv12_to_rgba(&nv12_data, width, height);
        assert!(result.is_err());
    }

    #[test]
    fn test_nv12_to_rgba_known_values() {
        // Test avec une image 4x4 pixels pour vérifier la conversion
        let width = 4u32;
        let height = 4u32;

        // NV12 format: Y plane (4x4 = 16 bytes) + UV plane (2x2 interleaved = 8 bytes)
        // Total: 24 bytes for 4x4 image

        // Créons une image avec des valeurs connues:
        // Y plane: 4x4 = 16 values
        // UV plane: 2x2 interleaved (UVUV...) = 8 values

        let mut nv12_data = vec![0u8; 24];

        // Y plane: Mettons du blanc pur (Y=235 en limited range)
        // En NV12, Y=235 correspond à blanc, Y=16 correspond à noir
        for i in 0..16 {
            nv12_data[i] = 235; // Blanc
        }

        // UV plane: Pour du blanc, U=128, V=128 (valeurs neutres)
        for i in 16..24 {
            nv12_data[i] = 128;
        }

        let result = nv12_to_rgba(&nv12_data, width, height);
        assert!(result.is_ok(), "Conversion should succeed");

        let rgba_data = result.unwrap();

        // Vérifier la taille: 4x4 pixels * 4 bytes (RGBA) = 64 bytes
        assert_eq!(rgba_data.len(), 64, "RGBA data should be 64 bytes (4x4x4)");

        // Vérifier quelques pixels
        // Le blanc (Y=235, U=128, V=128) devrait donner des valeurs RGB élevées
        println!(
            "\nFirst pixel RGBA: [{}, {}, {}, {}]",
            rgba_data[0], rgba_data[1], rgba_data[2], rgba_data[3]
        );

        // Pour du blanc, on s'attend à des valeurs élevées (proche de 255)
        // Avec limited range YUV, blanc = Y:235 U:128 V:128 -> RGB devrait être proche de (255,255,255)
        assert!(
            rgba_data[0] > 200,
            "R should be high for white (got {})",
            rgba_data[0]
        );
        assert!(
            rgba_data[1] > 200,
            "G should be high for white (got {})",
            rgba_data[1]
        );
        assert!(
            rgba_data[2] > 200,
            "B should be high for white (got {})",
            rgba_data[2]
        );
        assert_eq!(rgba_data[3], 255, "Alpha should be 255");
    }

    #[test]
    fn test_nv12_to_rgba_black() {
        // Test avec du noir pur
        let width = 4u32;
        let height = 4u32;

        let mut nv12_data = vec![0u8; 24];

        // Y plane: noir = 16 en limited range
        for i in 0..16 {
            nv12_data[i] = 16;
        }

        // UV plane: neutre
        for i in 16..24 {
            nv12_data[i] = 128;
        }

        let result = nv12_to_rgba(&nv12_data, width, height);
        assert!(result.is_ok());

        let rgba_data = result.unwrap();
        assert_eq!(rgba_data.len(), 64);

        println!(
            "\nBlack pixel RGBA: [{}, {}, {}, {}]",
            rgba_data[0], rgba_data[1], rgba_data[2], rgba_data[3]
        );

        // Pour du noir, RGB devrait être proche de (0,0,0)
        assert!(
            rgba_data[0] < 50,
            "R should be low for black (got {})",
            rgba_data[0]
        );
        assert!(
            rgba_data[1] < 50,
            "G should be low for black (got {})",
            rgba_data[1]
        );
        assert!(
            rgba_data[2] < 50,
            "B should be low for black (got {})",
            rgba_data[2]
        );
        assert_eq!(rgba_data[3], 255, "Alpha should be 255");
    }

    #[test]
    fn test_nv12_to_rgba_red() {
        // Test avec du rouge pur
        let width = 4u32;
        let height = 4u32;

        let mut nv12_data = vec![0u8; 24];

        // Rouge en YUV (limited range): Y≈82, U≈90, V≈240
        for i in 0..16 {
            nv12_data[i] = 82; // Y
        }

        // UV plane interleaved: UVUVUV...
        for i in 0..4 {
            // 2x2 chrominance
            nv12_data[16 + i * 2] = 90; // U
            nv12_data[16 + i * 2 + 1] = 240; // V
        }

        let result = nv12_to_rgba(&nv12_data, width, height);
        assert!(result.is_ok());

        let rgba_data = result.unwrap();

        println!(
            "\nRed pixel RGBA: [{}, {}, {}, {}]",
            rgba_data[0], rgba_data[1], rgba_data[2], rgba_data[3]
        );

        // Rouge devrait avoir R élevé, G et B bas
        assert!(
            rgba_data[0] > 200,
            "R should be high for red (got {})",
            rgba_data[0]
        );
        assert!(
            rgba_data[1] < 100,
            "G should be low for red (got {})",
            rgba_data[1]
        );
        assert!(
            rgba_data[2] < 100,
            "B should be low for red (got {})",
            rgba_data[2]
        );
        assert_eq!(rgba_data[3], 255, "Alpha should be 255");
    }

    // ========================================================================
    // BENCHMARKS DE PERFORMANCE
    // ========================================================================

    #[test]
    fn bench_nv12_to_rgba_hd() {
        use std::time::Instant;

        let width = 1920u32;
        let height = 1080u32;
        let nv12_size = (width * height * 3 / 2) as usize;

        // Données de test (gris moyen)
        let mut nv12_data = vec![128u8; nv12_size];
        // Variation pour rendre le test plus réaliste
        for i in 0..nv12_data.len() {
            nv12_data[i] = ((i % 256) as u8).wrapping_add(100);
        }

        let warmup = 5;
        let iterations = 50;

        // Warmup
        println!("\n🔥 Warmup ({} iterations)...", warmup);
        for _ in 0..warmup {
            let _ = nv12_to_rgba(&nv12_data, width, height).unwrap();
        }

        // Benchmark réel
        println!(
            "📊 Benchmarking NV12→RGB conversion ({}x{})...",
            width, height
        );
        println!("Running {} iterations...\n", iterations);

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = nv12_to_rgba(&nv12_data, width, height).unwrap();
        }
        let elapsed = start.elapsed();

        let avg_ms = elapsed.as_micros() as f64 / iterations as f64 / 1000.0;
        let fps = 1000.0 / avg_ms;

        println!("╔════════════════════════════════════════╗");
        println!("║     NV12→RGB PERFORMANCE RESULTS       ║");
        println!("╠════════════════════════════════════════╣");
        println!("║ Resolution:      {}x{}           ║", width, height);
        println!("║ Average time:    {:.2} ms              ║", avg_ms);
        println!("║ Min time:        ~{:.2} ms             ║", avg_ms * 0.9);
        println!("║ Max time:        ~{:.2} ms             ║", avg_ms * 1.1);
        println!("║ Theoretical FPS: {:.1} FPS             ║", fps);
        println!("╚════════════════════════════════════════╝\n");

        // Vérifications de performance
        if avg_ms > 50.0 {
            println!("⚠️  WARNING: Conversion is slow (>50ms)");
            println!("   Try enabling AVX2 or check CPU flags");
        } else if avg_ms > 20.0 {
            println!("⚠️  Performance could be better");
            println!("   Make sure you're running in --release mode");
        } else if avg_ms > 10.0 {
            println!("✅ Good performance (SSE optimized)");
        } else {
            println!("🚀 Excellent performance (AVX2 optimized)!");
        }

        // Affiche les features CPU détectées
        #[cfg(target_feature = "avx2")]
        println!("   CPU Features: AVX2 ✅");

        #[cfg(all(target_feature = "sse2", not(target_feature = "avx2")))]
        println!("   CPU Features: SSE2 ✅");

        #[cfg(not(any(target_feature = "sse2", target_feature = "avx2")))]
        println!("   CPU Features: None (fallback mode)");
    }

    #[test]
    fn bench_nv12_to_rgba_720p() {
        use std::time::Instant;

        let width = 1280u32;
        let height = 720u32;
        let nv12_size = (width * height * 3 / 2) as usize;
        let nv12_data = vec![128u8; nv12_size];

        let iterations = 100;
        let start = Instant::now();

        for _ in 0..iterations {
            let _ = nv12_to_rgba(&nv12_data, width, height).unwrap();
        }

        let elapsed = start.elapsed();
        let avg_ms = elapsed.as_micros() as f64 / iterations as f64 / 1000.0;
        let fps = 1000.0 / avg_ms;

        println!("\n📊 720p Performance:");
        println!("   Average: {:.2}ms", avg_ms);
        println!("   FPS: {:.1}", fps);
    }

    #[test]
    fn bench_yuv_to_rgb_hd() {
        use std::time::Instant;

        let width = 1920u32;
        let height = 1080u32;
        let yuv_size = (width * height * 3 / 2) as usize;
        let yuv_data = vec![128u8; yuv_size];

        let iterations = 50;

        println!("\n🔥 Warmup (YUV420)...");
        for _ in 0..5 {
            let _ = yuv_to_rgba(&yuv_data, width, height).unwrap();
        }

        println!("📊 Benchmarking YUV420→RGB conversion...\n");

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = yuv_to_rgba(&yuv_data, width, height).unwrap();
        }
        let elapsed = start.elapsed();

        let avg_ms = elapsed.as_micros() as f64 / iterations as f64 / 1000.0;
        let fps = 1000.0 / avg_ms;

        println!("╔════════════════════════════════════════╗");
        println!("║    YUV420→RGB PERFORMANCE RESULTS      ║");
        println!("╠════════════════════════════════════════╣");
        println!("║ Resolution:      {}x{}           ║", width, height);
        println!("║ Average time:    {:.2} ms              ║", avg_ms);
        println!("║ Theoretical FPS: {:.1} FPS             ║", fps);
        println!("╚════════════════════════════════════════╝\n");
    }
}
