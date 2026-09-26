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

typedef struct {
    uint32_t move_code;
    int32_t score;
    int32_t rank;
} FathomRootMove;

static int32_t copy_root_moves(
    const struct TbRootMoves *source,
    FathomRootMove *target,
    uint32_t capacity)
{
    if (source->size > capacity)
        return -1;

    for (uint32_t i = 0; i < source->size; ++i) {
        target[i].move_code = (uint32_t)source->moves[i].move;
        target[i].score = source->moves[i].tbScore;
        target[i].rank = source->moves[i].tbRank;
    }

    return (int32_t)source->size;
}

int32_t fathom_probe_root_dtz(
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
    uint8_t white_to_move,
    uint8_t has_repeated,
    uint8_t use_rule50,
    FathomRootMove *out_moves,
    uint32_t capacity)
{
    if (out_moves == NULL || capacity == 0)
        return -1;

    struct TbRootMoves results = {0};

    int success = tb_probe_root_dtz(
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
        white_to_move != 0,
        has_repeated != 0,
        use_rule50 != 0,
        &results
    );

    if (!success)
        return -1;

    return copy_root_moves(&results, out_moves, capacity);
}

int32_t fathom_probe_root_wdl(
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
    uint8_t white_to_move,
    uint8_t use_rule50,
    FathomRootMove *out_moves,
    uint32_t capacity)
{
    if (out_moves == NULL || capacity == 0)
        return -1;

    struct TbRootMoves results = {0};

    int success = tb_probe_root_wdl(
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
        white_to_move != 0,
        use_rule50 != 0,
        &results
    );

    if (!success)
        return -1;

    return copy_root_moves(&results, out_moves, capacity);
}

#ifdef __cplusplus
}
#endif