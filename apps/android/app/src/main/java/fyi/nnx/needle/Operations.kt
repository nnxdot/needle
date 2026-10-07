package fyi.nnx.needle

import kotlinx.coroutines.CancellationException

/** Completion runs on success, failure, and cancellation; cancellation keeps its meaning. */
internal suspend fun <T> performOperation(
    work: suspend () -> T,
    done: (T) -> Unit,
    failed: (Throwable) -> Unit,
    finished: () -> Unit,
) {
    try {
        done(work())
    } catch (cancelled: CancellationException) {
        throw cancelled
    } catch (error: Exception) {
        failed(error)
    } finally {
        finished()
    }
}
