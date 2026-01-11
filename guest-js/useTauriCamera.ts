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
  stop: () => Promise<void>
  getLatestFrame: () => FrameEvent | null
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
  let first_render = true
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
      // Ensure texture is initialized on first render for each new stream
      first_render = true

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

      console.log('[useTauriCamera] Creating camera stream...')

      // Setup frame rendering logic
      let pendingRender = false
      let lastRenderedFrameId = -1
      let latestPendingFrame: FrameEvent | null = null
      let frameDropCount = 0

      /**
       * Schedules a render for the latest frame.
       * Uses requestAnimationFrame to render in sync with the display refresh rate.
       * Drops frames that arrive while a render is pending to prevent memory buildup.
       * NO Promise returned to avoid memory leaks from unawaited closures.
       */
      const addFrame = (frame: FrameEvent): void => {
        // If we already have a render scheduled, just replace the pending frame
        if (pendingRender) {
          if (latestPendingFrame && latestPendingFrame.frameId !== frame.frameId) {
            frameDropCount++
            if (frameDropCount % 10 === 0) {
              console.log(`[Frame Drop] Dropped ${frameDropCount} frames total`)
            }
          }
          latestPendingFrame = frame
          return
        }

        // Skip if this frame was already rendered
        if (frame.frameId === lastRenderedFrameId) {
          return
        }

        pendingRender = true
        latestPendingFrame = frame

        // Schedule render without creating a Promise
        requestAnimationFrame(() => {
          // Use the latest frame (might have been updated while waiting)
          const frameToRender = latestPendingFrame
          latestPendingFrame = null

          if (!state.isStreaming || !frameToRender) {
            pendingRender = false
            return
          }

          const renderStart = performance.now()
          lastRenderedFrameId = frameToRender.frameId

            try {
              if (webglRenderer) {
                // WebGL rendering
                console.log(`[WebGL] Rendering frame #${frameToRender.frameId}`)
                const gl = webglRenderer.gl
                const program = webglRenderer.program
                const texture = webglRenderer.texture

                // Check data size
                const expectedSize = frameToRender.width * frameToRender.height * 4
                if (frameToRender.data.length !== expectedSize) {
                  console.error(`[WebGL] Data size mismatch! Expected ${expectedSize}, got ${frameToRender.data.length}`)
                } else {
                  // Update texture with frame data
                  gl.bindTexture(gl.TEXTURE_2D, texture)
                  if(first_render){
                  gl.texImage2D(
                    gl.TEXTURE_2D,
                    0,
                    gl.RGBA,
                    frameToRender.width,
                    frameToRender.height,
                    0,
                    gl.RGBA,
                    gl.UNSIGNED_BYTE,
                    frameToRender.data
                  )
                  first_render = false
                }
                else{
                  gl.texSubImage2D(
                    gl.TEXTURE_2D,
                    0,
                    0,
                    0,
                    frameToRender.width,
                                      frameToRender.height,
                    gl.RGBA,
                    gl.UNSIGNED_BYTE,
                    frameToRender.data
                    )
                  }                 


                  // Set horizontal flip
                  const flipLocation = gl.getUniformLocation(program, 'u_flipHorizontal')
                  gl.uniform1i(flipLocation, options?.flipHorizontal ? 1 : 0)

                  // Draw
                  gl.viewport(0, 0, canvas.width, canvas.height)
                  gl.clearColor(0, 0, 0, 1)
                  gl.clear(gl.COLOR_BUFFER_BIT)
                  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4)

                  const renderTime = performance.now() - renderStart
                  console.log(`[WebGL] Frame #${frameToRender.frameId} rendered in ${renderTime.toFixed(2)}ms`)
                }
              } else {
                // Canvas 2D rendering (fallback)
                console.log(`[Canvas2D] Rendering frame #${frameToRender.frameId}`)
                // Note: renderFrameToCanvas serait importé de core.ts
                // renderFrameToCanvas(canvas, frameToRender, { flipHorizontal: options?.flipHorizontal })
              }
            } catch (error) {
              console.error('[Render] Error:', error)
              options?.onError?.(error as Error)
            } finally {
              // Always clear the pending flag
              pendingRender = false
            }
        })
      }

      // Create the stream - frames trigger rendering via addFrame()
      state.currentStream = await createCameraStream(deviceId, {
        onFrame: (frame) => {
          console.log(`[useTauriCamera] Frame #${frame.frameId} received - ${frame.width}x${frame.height}, format: ${frame.format}`)
          
          // Schedule async rendering for this frame
          addFrame(frame)
          
          // Call user callback
          options?.onFrame?.(frame)
        },
        onError: (error) => {
          console.error('[useTauriCamera] Stream error:', error)
          options?.onError?.(error)
        },
      })

      console.log('[useTauriCamera] Camera stream created successfully')

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
    // Reset texture init flag for next start
    first_render = true
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
