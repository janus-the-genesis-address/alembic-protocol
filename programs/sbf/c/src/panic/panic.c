/**
 * @brief Example C-based SBF program that prints out the parameters
 * passed to it
 */
#include <Alembic_sdk.h>

extern uint64_t entrypoint(const uint8_t *input) {
  sol_panic();
  return SUCCESS;
}
