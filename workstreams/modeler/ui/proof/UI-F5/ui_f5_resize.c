/* UI-F5: resize the Isolith window to WxH on DISPLAY (no WM present). */
#include <X11/Xlib.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc < 4) { fprintf(stderr, "usage: %s DISPLAY W H\n", argv[0]); return 2; }
    Display *d = XOpenDisplay(argv[1]);
    if (!d) { fprintf(stderr, "cannot open display %s\n", argv[1]); return 1; }
    int W = atoi(argv[2]), H = atoi(argv[3]);
    Window root = DefaultRootWindow(d);
    Window rr, pr, *ch = NULL; unsigned int n = 0;
    if (!XQueryTree(d, root, &rr, &pr, &ch, &n)) return 1;
    for (unsigned int i = 0; i < n; i++) {
        char *nm = NULL;
        if (XFetchName(d, ch[i], &nm) && nm) {
            if (strstr(nm, "Isolith") || strstr(nm, "isolith")) {
                XResizeWindow(d, ch[i], (unsigned int)W, (unsigned int)H);
                if (nm) XFree(nm);
                XFlush(d);
                usleep(300000);
                return 0;
            }
            if (nm) XFree(nm);
        }
    }
    fprintf(stderr, "Isolith window not found\n");
    return 1;
}
