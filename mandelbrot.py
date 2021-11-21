#!/usr/bin/env python3

WIDTH = 2048
HEIGHT = 2048

# WIDTH = 960
# HEIGHT = 540

# WIDTH = 31
# HEIGHT = 31

# Y_MIN = -1.5
# Y_MAX = 1.5
# X_CENTER = -0.75

# # WIDTH / HEIGHT = _x_width / _y_height
# _aspect_ratio = WIDTH / HEIGHT
# _y_height = Y_MAX - Y_MIN
# _x_width = _aspect_ratio * _y_height
# X_MIN = X_CENTER - _x_width / 2
# X_MAX = X_CENTER + _x_width / 2

X_MIN = -2
X_MAX = 1
Y_MIN = -1.5
Y_MAX = 1.5

MAX_ITERATIONS = 255


def main():
    with open(f"mandelbrot_{WIDTH}x{HEIGHT}_{X_MIN}_{X_MAX}_{Y_MIN}_{Y_MAX}_{MAX_ITERATIONS}.ppm", "bw") as f:
        f.write(bytes(f'P6 {WIDTH} {HEIGHT} 255 ', encoding='ascii'))
        for row in range(HEIGHT):
            for col in range(WIDTH):
                x = map_inclusive_ranges(0, WIDTH - 1, X_MIN, X_MAX, col)
                y = map_inclusive_ranges(0, HEIGHT - 1, Y_MIN, Y_MAX, row)
                c = x + y * 1j
                iters = iterate(c)
                if iters == MAX_ITERATIONS:
                    f.write(bytes([0, 0, 0]))
                else:
                    gray = iters
                    f.write(bytes([gray] * 3))


def iterate(c):
    zn = 0
    for n in range(MAX_ITERATIONS):
        if abs(zn) > 2:
            return n
        else:
            zn = zn ** 2 + c

    return MAX_ITERATIONS


def map_inclusive_ranges(src_min, src_max, dest_min, dest_max, src_val):
    fraction_in_src = (src_val - src_min) / (src_max - src_min)
    return (dest_max - dest_min) * fraction_in_src + dest_min


if __name__ == '__main__':
    main()
