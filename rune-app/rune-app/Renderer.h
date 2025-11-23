#ifndef Renderer_h
#define Renderer_h


@interface Renderer : NSObject<MTKViewDelegate>
- (nonnull instancetype)initWithMetalKitView:(nonnull MTKView *)mtkView;

- (void)populateViewRect:(CefRect&)rect;

- (void)populateScreenInfo:(CefScreenInfo&)screenInfo;
    
- (void)paintType:(CefRenderHandler::PaintElementType)type
       dirtyRects:(const CefRenderHandler::RectList)dirtyRects
           buffer:(nonnull const void*)buffer
            width:(int)width
           height:(int)height;

- (void)setBrowser:(CefRefPtr<CefBrowser>)browser;

@end

#endif /* Renderer_h */
