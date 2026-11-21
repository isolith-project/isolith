/* probe_focus.c - variant of xtest_inject that sets input focus (guarded,
 * non-fatal) before sending Ctrl+K, to test whether focus is why the palette
 * does not open under bare Xvfb. Modes: AND / PAL / ORB (same as xtest_inject). */
#include <X11/Xlib.h>
#include <X11/keysym.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>

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
            found = root; XFree(name); return found;
        }
        XFree(name);
    }
    if (!XQueryTree(d, root, &r, &parent, &kids, &n)) return 0;
    for (unsigned int i = 0; i < n && !found; i++) found = find_isolith(d, kids[i]);
    if (kids) XFree(kids);
    return found;
}

static void send_key(Display *d, KeySym sym, int ctrl) {
    KeyCode kc = XKeysymToKeycode(d, sym);
    KeyCode ctrl_kc = XKeysymToKeycode(d, XK_Control_L);
    if (ctrl) { XTestFakeKeyEvent(d, ctrl_kc, True, 0); usleep(80000); }
    XTestFakeKeyEvent(d, kc, True, 0);
    usleep(80000);
    XTestFakeKeyEvent(d, kc, False, 0);
    usleep(80000);
    if (ctrl) { XTestFakeKeyEvent(d, ctrl_kc, False, 0); usleep(80000); }
    XFlush(d);
    usleep(200000);
}

static void drag(Display *d, int screen, int x1, int y1, int x2, int y2, int button) {
    XTestFakeMotionEvent(d, screen, x1, y1, 0);
    XFlush(d); usleep(150000);
    XTestFakeButtonEvent(d, button, True, 0);
    XFlush(d); usleep(150000);
    int steps = 16;
    for (int i = 1; i <= steps; i++) {
        int x = x1 + (x2 - x1) * i / steps;
        int y = y1 + (y2 - y1) * i / steps;
        XTestFakeMotionEvent(d, screen, x, y, 0);
        XFlush(d); usleep(50000);
    }
    XTestFakeButtonEvent(d, button, False, 0);
    XFlush(d); usleep(200000);
}

int main(int argc, char **argv) {
    if (argc < 3) { fprintf(stderr, "usage: %s <display> <AND|PAL|ORB>\n", argv[0]); return 2; }
    Display *d = XOpenDisplay(argv[1]);
    if (!d) { fprintf(stderr, "cannot open display %s\n", argv[1]); return 1; }
    int screen = DefaultScreen(d);
    Window root = DefaultRootWindow(d);
    Window app = 0;
    for (int i = 0; i < 120 && !app; i++) {
        app = find_isolith(d, root);
        if (!app) usleep(250000);
    }
    if (!app) { fprintf(stderr, "window not found\n"); XCloseDisplay(d); return 1; }
    XFlush(d);
    usleep(500000);
    /* guarded focus set - the one thing bare-Xvfb might be missing */
    XSetInputFocus(d, app, RevertToPointerRoot, CurrentTime);
    XFlush(d);
    usleep(200000);
    int cx = 600, cy = 395;
    const char *mode = argv[2];
    XTestFakeMotionEvent(d, screen, cx, cy, 0);
    XFlush(d); usleep(150000);
    if (!strcmp(mode, "PAL")) {
        send_key(d, XK_k, 1);
    } else if (!strcmp(mode, "ORB")) {
        drag(d, screen, cx - 130, cy - 40, cx + 130, cy + 60, 1);
    }
    usleep(800000);
    XCloseDisplay(d);
    return 0;
}
