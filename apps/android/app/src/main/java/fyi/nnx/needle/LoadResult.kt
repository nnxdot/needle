package fyi.nnx.needle

import kotlinx.coroutines.CancellationException

internal sealed interface LoadResult<out T> {
    data object Loading : LoadResult<Nothing>
    data class Ready<T>(val data: T) : LoadResult<T>
    data class Failed(val error: Throwable) : LoadResult<Nothing>
}

internal fun <T> loadAttempt(load: () -> T): LoadResult<T> = try {
    LoadResult.Ready(load())
} catch (cancelled: CancellationException) {
    throw cancelled
} catch (error: Exception) {
    LoadResult.Failed(error)
}
