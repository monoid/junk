#include <string.h>
#include <iostream>
#include <nmmintrin.h>

int main() {
	__m128i a = _mm_setr_epi16(3, 4, 5, 4, 5, 6, 7, 8);
	__m128i b = _mm_setr_epi16(5, 4, 3, 4, 5, 6, 7, 8);

	__m128i val;
	uint16_t r[8];

       	val = _mm_cmpestrm(a, 16, b, 16, _SIDD_CMP_EQUAL_ANY | _SIDD_UWORD_OPS);
       	memcpy(r, &val, sizeof(val));
	std::cout << 0 << ' ' << r[0] << std::endl;

	val = _mm_cmpestrm(a, 16, b, 16, _SIDD_CMP_RANGES | _SIDD_UWORD_OPS);
       	memcpy(r, &val, sizeof(val));
	std::cout << 1 << ' ' << r[0] << std::endl;

	val = _mm_cmpestrm(a, 16, b, 16, _SIDD_CMP_EQUAL_EACH | _SIDD_UWORD_OPS);
       	memcpy(r, &val, sizeof(val));
	std::cout << 2 << ' ' << r[0] << std::endl;

	val = _mm_cmpestrm(a, 3, b, 16, _SIDD_CMP_EQUAL_ORDERED | _SIDD_UWORD_OPS);
       	memcpy(r, &val, sizeof(val));
	std::cout << 4 << ' ' << r[0] << std::endl;
	return 0;

}
