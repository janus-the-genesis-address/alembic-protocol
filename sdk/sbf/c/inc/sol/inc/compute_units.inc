#pragma once
/**
 * @brief Alembic logging utilities
 */

#include <TACHYON/types.h>
#include <TACHYON/string.h>
#include <TACHYON/entrypoint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Prints a string to stdout
 */
@SYSCALL uint64_t sol_remaining_compute_units();

#ifdef SOL_TEST
/**
 * Stub functions when building tests
 */

uint64_t sol_remaining_compute_units() {
  return UINT64_MAX;
}
#endif

#ifdef __cplusplus
}
#endif

/**@}*/
