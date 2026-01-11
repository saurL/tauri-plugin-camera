import {
  initialize,
  getAvailableCameras as getCameras,
  createCameraStream,
  type CameraDeviceInfo,
  type FrameEvent
} from './core'

export type { CameraDeviceInfo, FrameEvent }

interface CameraStreamController {
  sessionId: string
  stop: () => void
  canvas: HTMLCanvasElement
  getFrameInfo: () => {
    frameId: number
    fps: number
    width: number
    height: number
  } | null
}

interface WebGLRenderer {
  gl: WebGLRenderingContext
  program: WebGLProgram
  texture: WebGLTexture
  cleanup: () => void
}

interface CameraState {
  cameras: CameraDeviceInfo[]
  isLoading: boolean
  error: string | null
  isStreaming: boolean
  currentStream: CameraStreamController | null
}

export const useTauriCamera = () => {
  const state: CameraState = {
    cameras: [],
    isLoading: false,
    error: null,
    isStreaming: false,
    currentStream: null
  }

  let webglRenderer: WebGLRenderer | null = null

  // Initialiser le système de caméra
  const initializeCameraSystem = async (): Promise<void> => {
    console.log('Initializing camera system...')
    try {
      await initialize()
      console.log('Camera system initialized')
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      console.error('Failed to initialize camera system:', errorMsg)
      state.error = errorMsg
      throw e
    }
  }

  // Obtenir les caméras disponibles
  const getAvailableCameras = async (): Promise<CameraDeviceInfo[]> => {
    console.log('Getting available cameras...')
    state.isLoading = true
    state.error = null

    try {
      await initialize()
      const result = await getCameras()
      console.log('Available cameras:', result)
      state.cameras = result
      return state.cameras
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      console.error('Failed to get cameras:', errorMsg)
      state.error = errorMsg
      throw e
    } finally {
      state.isLoading = false
    }
  }
// Create a WebGL renderer for optimized display
const createWebGLRenderer = (canvas: HTMLCanvasElement): WebGLRenderer => {
  const gl = canvas.getContext('webgl', {
  alpha: false,           // No alpha channel = faster
  antialias: false,       // No antialiasing = faster
  depth: false,           // No depth buffer = save memory
  stencil: false,         // No stencil buffer = save memory
  preserveDrawingBuffer: false,  // No preservation = faster
  powerPreference: 'high-performance'  // Force high-performance GPU
}) as WebGLRenderingContext | null

  
  if (!gl) {
    throw new Error('WebGL not supported')
  }

  // Simple vertex shader to display a texture
  const vertexShaderSource = `
    attribute vec2 a_position;
    attribute vec2 a_texCoord;
    varying vec2 v_texCoord;

    void main() {
      gl_Position = vec4(a_position, 0.0, 1.0);
      v_texCoord = a_texCoord;
    }
  `

  // Fragment shader with horizontal flip support
  const fragmentShaderSource = `
    precision mediump float;
    varying vec2 v_texCoord;
    uniform sampler2D u_texture;
    uniform bool u_flipHorizontal;

    void main() {
      vec2 coord = v_texCoord;
      if (u_flipHorizontal) {
        coord.x = 1.0 - coord.x;
      }
      gl_FragColor = texture2D(u_texture, coord);
    }
  `

  // Compile shaders
  const vertexShader = gl.createShader(gl.VERTEX_SHADER)
  if (!vertexShader) throw new Error('Failed to create vertex shader')
  
  gl.shaderSource(vertexShader, vertexShaderSource)
  gl.compileShader(vertexShader)

  const fragmentShader = gl.createShader(gl.FRAGMENT_SHADER)
  if (!fragmentShader) throw new Error('Failed to create fragment shader')
  
  gl.shaderSource(fragmentShader, fragmentShaderSource)
  gl.compileShader(fragmentShader)

  // Create program
  const program = gl.createProgram()
  if (!program) throw new Error('Failed to create WebGL program')
  
  gl.attachShader(program, vertexShader)
  gl.attachShader(program, fragmentShader)
  gl.linkProgram(program)
  gl.useProgram(program)

  // Configure vertices (fullscreen rectangle)
  const vertices = new Float32Array([
    -1, -1,  0, 1,  // Bottom left
     1, -1,  1, 1,  // Bottom right
    -1,  1,  0, 0,  // Top left
     1,  1,  1, 0   // Top right
  ])

  const buffer = gl.createBuffer()
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
  gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW)

  const positionLocation = gl.getAttribLocation(program, 'a_position')
  const texCoordLocation = gl.getAttribLocation(program, 'a_texCoord')

  gl.enableVertexAttribArray(positionLocation)
  gl.vertexAttribPointer(positionLocation, 2, gl.FLOAT, false, 16, 0)

  gl.enableVertexAttribArray(texCoordLocation)
  gl.vertexAttribPointer(texCoordLocation, 2, gl.FLOAT, false, 16, 8)

  // Create texture
  const texture = gl.createTexture()
  if (!texture) throw new Error('Failed to create texture')
  
  gl.bindTexture(gl.TEXTURE_2D, texture)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR)

  return {
    gl,
    program,
    texture,
    cleanup: () => {
      gl.deleteTexture(texture)
      gl.deleteProgram(program)
      gl.deleteShader(vertexShader)
      gl.deleteShader(fragmentShader)
      gl.deleteBuffer(buffer)
    }
  }
}


  /**
   * Start camera streaming with optional WebGL rendering
   *
   * @param canvas - The HTMLCanvasElement to render to
   * @param deviceId - The camera device ID
   * @param options - Streaming options
   * @param options.flipHorizontal - Whether to flip the video horizontally (default: true)
   * @param options.useWebGL - Use WebGL for hardware-accelerated rendering (default: false)
   * @param options.onFrame - Callback function called for each frame
   * @param options.onError - Callback function called on error
   *
   * @returns A promise that resolves to a CameraStreamController
   *
   * @remarks
   * For optimal WebGL performance, ensure that the Rust backend sends frames in RGBA format.
   * If frames are in RGB8 format, uncomment the conversion code in the WebGL callback.
   *
   * @example
   * ```typescript
   * const camera = useTauriCamera()
   * const canvas = document.querySelector('canvas')!
   *
   * await camera.startStreaming(canvas, '0', {
   *   flipHorizontal: true,
   *   useWebGL: true,
   *   onFrame: (frame) => console.log('FPS:', camera.getFrameInfo()?.fps)
   * })
   * ```
   */
  const startStreaming = async (
    canvas: HTMLCanvasElement,
    deviceId: string,
    options?: {
      flipHorizontal?: boolean
      useWebGL?: boolean
      onFrame?: (frame: FrameEvent) => void
      onError?: (error: Error) => void
    }
  ): Promise<CameraStreamController> => {
    console.log('Starting streaming for device:', deviceId)
    state.isLoading = true
    state.error = null

    try {
      if (state.currentStream) {
        state.currentStream.stop()
        state.currentStream = null
      }

      if (webglRenderer) {
        webglRenderer.cleanup()
        webglRenderer = null
      }

      // Initialize WebGL if requested
      if (options?.useWebGL) {
        try {
          webglRenderer = createWebGLRenderer(canvas)
          console.log('WebGL renderer initialized')
        } catch (e) {
          console.warn('WebGL not available, falling back to Canvas 2D', e)
        }
      }

      // Wrapper for onFrame callback with WebGL
      // NOTE: For optimal WebGL performance, frame.data should be in RGBA format
      // If you need RGB8 to RGBA conversion, uncomment the conversion code below

      console.log('[useTauriCamera] Setting up frame callback, WebGL:', !!webglRenderer)

      const frameCallback = webglRenderer
        ? (frame: FrameEvent) => {
            console.log(`[WebGL] Frame #${frame.frameId} received - ${frame.width}x${frame.height}, ${frame.data.length} bytes, format: ${frame.format}`)

            const renderStart = performance.now()

            try {
              const gl = webglRenderer!.gl
              const program = webglRenderer!.program
              const texture = webglRenderer!.texture

              // Check data size
              const expectedSize = frame.width * frame.height * 4
              if (frame.data.length !== expectedSize) {
                console.error(`[WebGL] Data size mismatch! Expected ${expectedSize}, got ${frame.data.length}`)
                return
              }

              // Update texture directly with data (assuming RGBA)
              console.log('[WebGL] Binding texture and uploading data...')
              gl.bindTexture(gl.TEXTURE_2D, texture)
              gl.texImage2D(
                gl.TEXTURE_2D,
                0,
                gl.RGBA,
                frame.width,
                frame.height,
                0,
                gl.RGBA,
                gl.UNSIGNED_BYTE,
                frame.data
              )

              // Set horizontal flip
              const flipLocation = gl.getUniformLocation(program, 'u_flipHorizontal')
              gl.uniform1i(flipLocation, options?.flipHorizontal ? 1 : 0)

              // Draw
              console.log('[WebGL] Drawing...')
              gl.viewport(0, 0, canvas.width, canvas.height)
              gl.clearColor(0, 0, 0, 1)
              gl.clear(gl.COLOR_BUFFER_BIT)
              gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4)

              const renderTime = performance.now() - renderStart
              console.log(`[WebGL] Frame #${frame.frameId} rendered in ${renderTime.toFixed(2)}ms`)

              // Call user callback
              options?.onFrame?.(frame)
            } catch (error) {
              console.error('[WebGL] Render error:', error)
            }
          }
        : (frame: FrameEvent) => {
            console.log(`[Canvas2D] Frame #${frame.frameId} received - ${frame.width}x${frame.height}, format: ${frame.format}`)
            options?.onFrame?.(frame)
          }

      console.log('[useTauriCamera] Creating camera stream...')
      state.currentStream = await createCameraStream(canvas, deviceId, {
        flipHorizontal: options?.flipHorizontal ?? true,
        onFrame: frameCallback,
        onError: (error) => {
          console.error('[useTauriCamera] Stream error:', error)
          options?.onError?.(error)
        },
      })

      console.log('Streaming started successfully')
      state.isStreaming = true
      return state.currentStream
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      console.error('Failed to start streaming:', errorMsg)
      state.error = errorMsg
      throw e
    } finally {
      state.isLoading = false
    }
  }

  // Arrêter le streaming
  const stopStreaming = async () => {
    console.log('Stopping streaming...')
    if (state.currentStream) {
      await state.currentStream.stop()
      state.currentStream = null
      state.isStreaming = false
    }
    if (webglRenderer) {
      webglRenderer.cleanup()
      webglRenderer = null
    }
    console.log('Streaming stopped')
  }

  const getFrameInfo = () => {
    if (!state.currentStream) return null
    return state.currentStream.getFrameInfo()
  }

  return {
    get cameras() { return state.cameras },
    get isLoading() { return state.isLoading },
    get isStreaming() { return state.isStreaming },
    get error() { return state.error },
    get currentStream() { return state.currentStream },
    initializeCameraSystem,
    getAvailableCameras,
    startStreaming,
    stopStreaming,
    getFrameInfo,
  }
}
