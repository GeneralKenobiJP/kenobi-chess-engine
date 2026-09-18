#include <stdint.h>
#include "tbprobe.h"

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Initialize Fathom.
 *
 * Returns:
 *   1 on successful initialization
 *   0 on failure
 *
 * Note that Fathom may return success while finding zero tablebase files.
 * Check fathom_largest() afterwards.
 */
int32_t fathom_init(const char *path)
{
    return tb_init(path) ? 1 : 0;
}


/*
 * Release tablebase resources.
 */
void fathom_free(void)
{
    tb_free();
}


/*
 * Maximum tablebase cardinality actually found.
 *
 * Example:
 *   0 -> no WDL tablebases found
 *   3 -> tables up through 3 pieces found
 *   5 -> tables up through 5 pieces found
 */
uint32_t fathom_largest(void)
{
    return (uint32_t)TB_LARGEST;
}


/*
 * Public WDL wrapper.
 *
 * white/black are color occupancy bitboards.
 *
 * kings/queens/rooks/bishops/knights/pawns are
 * piece-type occupancy bitboards containing both colors.
 */
uint32_t fathom_probe_wdl(
    uint64_t white,
    uint64_t black,
    uint64_t kings,
    uint64_t queens,
    uint64_t rooks,
    uint64_t bishops,
    uint64_t knights,
    uint64_t pawns,
    uint32_t rule50,
    uint32_t castling,
    uint32_t ep,
    uint8_t white_to_move)
{
    return (uint32_t)tb_probe_wdl(
        white,
        black,
        kings,
        queens,
        rooks,
        bishops,
        knights,
        pawns,
        rule50,
        castling,
        ep,
        white_to_move != 0
    );
}

#ifdef __cplusplus
}
#endif