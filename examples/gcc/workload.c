#include <stdio.h>
#include <math.h>

#define N 4096

static double A[N][N];

void fill(void) {
    for (int i = 0; i < N; i++)
        for (int j = 0; j < N; j++)
            A[i][j] = sin((double)(i * N + j));
}

double sum(void) {
    double s = 0.0;
    for (int i = 0; i < N; i++)
        for (int j = 0; j < N; j++)
            s += A[i][j];
    return s;
}

int main(void) {
    fill();
    printf("sum: %f\n", sum());
    return 0;
}
