/*
 * UI-A8 proof helper: minimal XTEST input injector for deterministic
 * screenshots of the real Isolith binary under xvfb-run on the merged head.
 *
 * Build (host has libX11 headers + libXtst .so.6, no XTest.h, no xdotool):
 *   gcc -O2 -o xtest_inject xtest_inject.c -lX11 -Wl,-l:libXtst.so.6
 *
 * Usage: xtest_inject <display> <mode>
 *   mode AND = nothing (just ensure pointer over the app; captures the shell)
 *   mode PAL = Ctrl+K (opens the command palette overlay)
 *   mode ORB = left-drag orbit inside the viewport region (camera moves)
 *   mode PAN = middle-drag pan inside the viewport
 *   mode ZMM = scroll wheel (zoom) over the viewport
 *
 * The app window is 1280x800 at +0+0 on the xvfb screen; the viewport region
 * spans x in [240,964], y in [32,758] (layout card A4). XTEST fake events are
 * indistinguishable from real hardware events to winit/egui, so the resulting
 * frame is a real app render, not a composited stub.
 *
 * Focus handling under bare Xvfb: there is no WM, so the input focus stays on
 * PointerRoot (keyboard events go to whatever window is under the pointer).
 * We never call XSetInputFocus (it can BadMatch on a freshly mapped GL window
 * and abort the whole injection); instead we always warp the pointer over the
 * app window first, which routes key events (Ctrl+K) and pointer events alike
 * to the app. Best-effort and never fatal.
 */
#include <X11/Xlib.h>
#include <X11/keysym.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>

/* libXtst prototypes (headers absent on host; ABI stable for these three). */
extern int XTestFakeKeyEvent(Display *d, unsigned int keycode, Bool is_press,
                             unsigned long delay);
extern int XTestFakeButtonEvent(Display *d, unsigned int button, Bool is_press,
                                unsigned long delay);
extern int XTestFakeMotionEvent(Display *d, int screen, int x, int y,
                                unsigned long delay);

static Window find_isolith(Display *d, Window root) {
    static Window found = 0;
    Window r, parent;
    Window *kids = NULL;
    unsigned int n = 0;
    char *name = NULL;
    if (found) return found;
    if (XFetchName(d, root, &name) && name) {
        if (strcasestr(name, "isol") || strcasestr(name, "modeler")) {
            found = root;
            XFree(name);
            return found;
        }
        XFree(name);
    }
    if (!XQueryTree(d, root, &r, &parent, &kids, &n)) return 0;
    for (unsigned int i = 0; i < n && !found; i++) {
        found = find_isolith(d, kids[i]);
    }
    if (kids) XFree(kids);
    return found;
}

static void send_key(Display *d, KeySym sym, int ctrl) {
    KeyCode kc = XKeysymToKeycode(d, sym);
    KeyCode ctrl_kc = XKeysymToKeycode(d, XK_Control_L);
    if (ctrl) { XTestFakeKeyEvent(d, ctrl_kc, True, 0); usleep(50000); }
    XTestFakeKeyEvent(d, kc, True, 0);
    usleep(50000);
    XTestFakeKeyEvent(d, kc, False, 0);
    usleep(50000);
    if (ctrl) { XTestFakeKeyEvent(d, ctrl_kc, False, 0); usleep(50000); }
    XFlush(d);
    usleep(120000);
}

static void drag(Display *d, int screen, int x1, int y1, int x2, int y2,
                 int button) {
    XTestFakeMotionEvent(d, screen, x1, y1, 0);
    XFlush(d);
    usleep(120000);
    XTestFakeButtonEvent(d, button, True, 0);
    XFlush(d);
    usleep(120000);
    /* interpolate so egui sees a real drag path */
    int steps = 16;
    for (int i = 1; i <= steps; i++) {
        int x = x1 + (x2 - x1) * i / steps;
        int y = y1 + (y2 - y1) * i / steps;
        XTestFakeMotionEvent(d, screen, x, y, 0);
        XFlush(d);
        usleep(40000);
    }
    XTestFakeButtonEvent(d, button, False, 0);
    XFlush(d);
    usleep(150000);
}

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: %s <display> <AND|PAL|ORB|PAN|ZMM>\n", argv[0]);
        return 2;
    }
    Display *d = XOpenDisplay(argv[1]);
    if (!d) { fprintf(stderr, "xtest_inject: cannot open display %s\n", argv[1]); return 1; }
    int screen = DefaultScreen(d);
    Window root = DefaultRootWindow(d);

    /* Wait for the app window to appear. */
    Window app = 0;
    for (int i = 0; i < 120 && !app; i++) {
        app = find_isolith(d, root);
        if (!app) usleep(250000);
    }
    if (!app) { fprintf(stderr, "xtest_inject: Isolith window not found\n"); XCloseDisplay(d); return 1; }
    XFlush(d);
    usleep(400000);

    const char *mode = argv[2];
    /* centre of the viewport region per layout card A4 */
    int cx = 600, cy = 395;
    /* Orbit drag start near viewport centre-toward left, end toward right.
     * Keep entirely inside the viewport x[240,964] y[32,758]. */
    int ox1 = cx - 130, oy1 = cy - 40;
    int ox2 = cx + 130, oy2 = cy + 60;
    /* Pan drag (middle button) slightly up-right. */
    int px1 = cx, py1 = cy;
    int px2 = cx + 90, py2 = cy + 50;

    if (!strcmp(mode, "AND")) {
        /* warp pointer into the viewport so keyboard would route there too */
        XTestFakeMotionEvent(d, screen, cx, cy, 0);
        XFlush(d);
    } else if (!strcmp(mode, "PAL")) {
        XTestFakeMotionEvent(d, screen, cx, cy, 0);
        XFlush(d);
        usleep(100000);
        send_key(d, XK_k, 1);
    } else if (!strcmp(mode, "ORB")) {
        drag(d, screen, ox1, oy1, ox2, oy2, 1);
    } else if (!strcmp(mode, "PAN")) {
        drag(d, screen, px1, py1, px2, py2, 2);
    } else if (!strcmp(mode, "ZMM")) {
        XTestFakeMotionEvent(d, screen, cx, cy, 0);
        XFlush(d);
        usleep(100000);
        for (int i = 0; i < 3; i++) {
            XTestFakeButtonEvent(d, 4, True, 0); /* wheel up = zoom in */
            XTestFakeButtonEvent(d, 4, False, 0);
            usleep(80000);
        }
        XFlush(d);
    } else {
        fprintf(stderr, "unknown mode %s\n", mode);
        XCloseDisplay(d);
        return 2;
    }
    usleep(600000); /* let the frame settle before xwd captures */
    XCloseDisplay(d);
    return 0;
}
