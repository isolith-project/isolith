// UI-F1 wave-F: deterministic real-binary screenshot of the drafting sheet
// panel (sheet tree + sheet properties), byte-identical across two
// independent app launches.
//
// The drafting region is the lower 48% of the properties (right) column at
// the default 1280x800 window. The capture needs no clicks: the first-frame
// seed opens the committed bracket drawing (A4, third-angle, three views) and
// the panel shows exactly that - Sheet 1 row, Add sheet, size/scale/
// projection selectors, and the view-placement legality line. The pointer is
// parked neutral (away from every panel).
//
// No XTest dev header is installed on this host (only libXtst.so.6), so the
// single XTest function we need is declared explicitly below rather than
// pulled from X11/extensions/XTest.h; linked with `-Wl,-l:libXtst.so.6`.

#include <X11/Xlib.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// XTestFakeMotionEvent(Display*, int screen, int x, int y, unsigned long delay)
extern int XTestFakeMotionEvent(Display *, int, int, int, unsigned long);

int main(int argc, char **argv) {
    if (argc < 5) {
        fprintf(stderr, "usage: %s DISPLAY move X Y\n", argv[0]);
        return 2;
    }
    Display *d = XOpenDisplay(argv[1]);
    if (!d) {
        fprintf(stderr, "inject: cannot open display %s\n", argv[1]);
        return 1;
    }
    if (strcmp(argv[2], "move") == 0) {
        int x = atoi(argv[3]);
        int y = atoi(argv[4]);
        XTestFakeMotionEvent(d, -1, x, y, CurrentTime);
        XFlush(d);
    } else {
        fprintf(stderr, "inject: unknown op %s\n", argv[2]);
        return 2;
    }
    XCloseDisplay(d);
    return 0;
}
