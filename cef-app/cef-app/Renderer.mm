#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Weverything"
#import "include/cef_app.h"
#import "include/cef_browser.h"
#import "include/cef_client.h"
#import "include/wrapper/cef_library_loader.h"
#pragma clang diagnostic pop


#import <Metal/Metal.h>
#import <MetalKit/MetalKit.h>
#import <Cocoa/Cocoa.h>
#import <QuartzCore/CAMetalLayer.h>
#import <simd/simd.h>
#import <mach/mach.h>
#import "Renderer.h"


struct ShaderParameters {
    simd::float2 mouse;
};


@implementation Renderer {
    NSDate *date;
    float fps;
    int i;
    id<MTLDevice> device;
    id<MTLCommandQueue> commandQueue;
    id<MTLTexture> texture;
    id<MTLComputePipelineState> computePipelineState;
    
    CGSize size;
    CefRefPtr<CefBrowser> browser;
}

- (nonnull instancetype)initWithMetalKitView:(nonnull MTKView *)mtkView
{
    self = [super init];
    if (self)
    {
        device = MTLCreateSystemDefaultDevice();
        mtkView.device = device;
        mtkView.clearColor = MTLClearColorMake(0,0,0,0);
        mtkView.framebufferOnly = false;
        size = mtkView.drawableSize;
        commandQueue = [device newCommandQueue];
        
        id<MTLLibrary> library = [device newDefaultLibrary];
        id<MTLFunction> computeShader = [library newFunctionWithName: @"compute_shader"];
        computePipelineState = [device newComputePipelineStateWithFunction:computeShader error: nil];
    }
    
    return self;
}

- (void)setBrowser:(CefRefPtr<CefBrowser>)_browser
{
    browser = _browser;
}


- (void)populateViewRect:(CefRect &)rect
{
    if (size.width > 0) {
        float deviceScaleFactor = [self getDeviceScaleFactor];
        rect.Set(0,0, (int)size.width / deviceScaleFactor, (int)size.height / deviceScaleFactor);
    }
}

- (float)getDeviceScaleFactor {
    return [[NSScreen mainScreen] backingScaleFactor];
}

- (void)populateScreenInfo:(CefScreenInfo&)screenInfo {
    screenInfo.device_scale_factor = [self getDeviceScaleFactor];
}

- (void)paintType:(CefRenderHandler::PaintElementType)type
       dirtyRects:(const CefRenderHandler::RectList)dirtyRects
           buffer:(const void*)buffer
            width:(int)width
           height:(int)height
{
    
    CefRenderHandler::RectList::const_iterator i = dirtyRects.begin();
    
    if(!texture || texture.width < width || texture.height < height){
        
        MTLTextureDescriptor *textureDescriptor = [[MTLTextureDescriptor alloc] init];
        textureDescriptor.pixelFormat = MTLPixelFormatBGRA8Unorm;
        textureDescriptor.width = width;
        textureDescriptor.height = height;
        texture = [device newTextureWithDescriptor:textureDescriptor];
        int x = 0;
        int y = 0;
        int w = width;
        int h = height;
        MTLRegion region = MTLRegionMake2D(x, y, w, h);
        [texture replaceRegion:region mipmapLevel:0 withBytes:(char*)buffer + (y * width + x) * 4 bytesPerRow:4 * width];
    } else {
        for (; i != dirtyRects.end(); ++i) {
            const CefRect& rect = *i;
            
            int x = rect.x;
            int y = rect.y;
            int w = rect.width;
            int h = rect.height;
            // NSLog(@"dirty x: %d, y: %d, w: %d, h: %d", x, y, w, h);
            MTLRegion region = MTLRegionMake2D(x, y, w, h);
            
            [texture
                replaceRegion:region
                mipmapLevel:0
                withBytes:(char*)buffer + (y * width + x) * 4
                bytesPerRow:4 * width
             ];
           
        }
    }
    
}

- (void)drawInMTKView:(nonnull MTKView *)view
{
    
    // The render pass descriptor references the texture into which Metal should draw
    MTLRenderPassDescriptor *renderPassDescriptor = view.currentRenderPassDescriptor;
    if (renderPassDescriptor == nil)
    {
        NSLog(@"currentRenderPassDescriptor failed");
    }

    i++;
    renderPassDescriptor.colorAttachments[0].clearColor =
        MTLClearColorMake(
                  sin(i / 2 / 3.14159 / 17)/2+0.5,
                  sin(i / 2 / 3.14159 / 20)/2+0.5,
                  sin(i / 2 / 3.14159 / 13)/2+0.5,
                  1);
    renderPassDescriptor.colorAttachments[0].loadAction = MTLLoadActionClear;
    renderPassDescriptor.colorAttachments[0].storeAction = MTLStoreActionStore;

    
    id<MTLCommandBuffer> commandBuffer = [commandQueue commandBuffer];
    
    id<MTLRenderCommandEncoder> commandEncoder = [commandBuffer renderCommandEncoderWithDescriptor:renderPassDescriptor];
    [commandEncoder endEncoding];
    
    id<CAMetalDrawable> drawable = view.currentDrawable;
    if (drawable != nil && computePipelineState != nil)
    {
        id<MTLTexture> drawingTexture = [drawable texture];
        
        if (drawingTexture != nil && texture != nil)
        {
           
            id<MTLComputeCommandEncoder> encoder = [commandBuffer computeCommandEncoder];
            if (encoder != nil)
            {
                ShaderParameters params;
               
                
                float scaleFactor = [self getDeviceScaleFactor];
                NSPoint screenPoint = [NSEvent mouseLocation];
                NSPoint windowPoint = [view.window convertPointFromScreen:screenPoint];
                NSPoint viewPoint = [view convertPoint:windowPoint fromView:nil];
                NSPoint flippedViewPoint;
                flippedViewPoint.x = viewPoint.x;
                flippedViewPoint.y = [view frame].size.height - viewPoint.y;
                
                params.mouse = simd::make_float2(flippedViewPoint.x * scaleFactor, flippedViewPoint.y * scaleFactor);

                
                
                [encoder setComputePipelineState:computePipelineState];
                
                [encoder
                    setBytes: &params
                    length: sizeof(params)
                    atIndex: 0
                 ];
                [encoder setTexture:texture atIndex:1];
                [encoder setTexture:drawingTexture atIndex:2];
                [encoder setTexture:drawingTexture atIndex:3];
              
                NSUInteger w = computePipelineState.threadExecutionWidth;
                NSUInteger h = computePipelineState.maxTotalThreadsPerThreadgroup / w;
                MTLSize threadsPerThreadgroup = MTLSizeMake(w, h, 1);
                
                MTLSize threadsPerGrid = MTLSizeMake(
                                                   texture.width,
                                                   texture.height,
                                                   1);
                [encoder
                    dispatchThreads:threadsPerGrid
                    threadsPerThreadgroup:threadsPerThreadgroup
                ];
                [encoder endEncoding];
                
            }
            
        }
    }
    
    [commandBuffer presentDrawable:drawable];
    [commandBuffer commit];
    
    if (date) {
        double frameTimeMs = -1000 * [date timeIntervalSinceNow];
        double smoothing = 0.9;
        
        fps = fps * smoothing + (1000 / frameTimeMs) * (1.0 - smoothing);
       // NSLog(@"fps: %f, frametime: %f", fps, frameTimeMs);
      
    }
    
    date = [NSDate date];
}

- (void)mtkView:(nonnull MTKView *)view drawableSizeWillChange:(CGSize)_size {
    size = _size;
}

@end
