// main.mm - Rune Scene macOS application entry point

#import <Cocoa/Cocoa.h>
#import "AppDelegate.h"

int main(int argc, const char * argv[]) {
    @autoreleasepool {
        [NSApplication sharedApplication];

        AppDelegate* appDelegate = [AppDelegate new];
        [NSApp setDelegate:appDelegate];

        // Run the main event loop
        [NSApp run];
    }
    return 0;
}
