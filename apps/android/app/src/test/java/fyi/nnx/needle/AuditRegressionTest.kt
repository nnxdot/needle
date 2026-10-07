package fyi.nnx.needle

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.async
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.withTimeout
import org.junit.Assert.*
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.IOException
import java.io.InputStream
import java.nio.file.Files

class AuditRegressionTest {
    @Test fun slowProviderCopiesLeaveTheCallingThreadResponsive() = runBlocking {
        val folder = Files.createTempDirectory("needle-slow-provider").toFile()
        try {
            val started = CompletableDeferred<Unit>()
            val callerThread = Thread.currentThread()
            val copy = async {
                copyFileOnIo(folder, "slow.wav") {
                    assertNotEquals(callerThread, Thread.currentThread())
                    started.complete(Unit)
                    Thread.sleep(300)
                    ByteArrayInputStream("complete".toByteArray())
                }
            }
            started.await()
            val start = System.nanoTime()
            withTimeout(100) { kotlinx.coroutines.yield() }
            assertFalse(copy.isCompleted)
            println("Slow provider: 300 ms read; caller yielded in ${(System.nanoTime()-start)/1_000_000.0} ms")
            assertEquals("complete", copy.await().readText())
        } finally { folder.deleteRecursively() }
    }
    @Test fun loadsDistinguishEmptyNullAndFailureAndCanRetry() {
        assertEquals(LoadResult.Ready(emptyList<String>()), loadAttempt { emptyList<String>() })
        assertEquals(LoadResult.Ready<String?>(null), loadAttempt<String?> { null })
        var offline = true
        val load = { if (offline) throw IOException("offline") else "loaded" }
        assertTrue(loadAttempt(load) is LoadResult.Failed)
        offline = false
        assertEquals(LoadResult.Ready("loaded"), loadAttempt(load))
        try { loadAttempt<Int> { throw CancellationException() }; fail("cancellation must propagate") }
        catch (_: CancellationException) { }
    }
    @Test fun identicalDisplayNamesKeepBothImportsAndFailureKeepsTheFirst() {
        val folder = Files.createTempDirectory("needle-copy").toFile()
        try {
            val first = copyFileAtomically(folder, "song.mp3", { ByteArrayInputStream("first".toByteArray()) })
            val second = copyFileAtomically(folder, "song.mp3", { ByteArrayInputStream("second".toByteArray()) })
            assertNotEquals(first, second)
            assertTrue(first.name.endsWith(".mp3"))
            assertEquals("first", first.readText())
            assertEquals("second", second.readText())
            try {
                copyFileAtomically(folder, "song.mp3", { object : InputStream() {
                    var reads = 0
                    override fun read(): Int = if (reads++ < 10) 65 else throw IOException("provider disconnected")
                } })
                fail("copy should fail")
            } catch (_: IOException) { }
            assertEquals(setOf(first.name, second.name), folder.list()!!.toSet())
            assertEquals("first", first.readText())
        } finally { folder.deleteRecursively() }
    }

    @Test fun cancellingACopyRemovesItsPartialFile() {
        val folder = Files.createTempDirectory("needle-cancel").toFile()
        try {
            var checks = 0
            try {
                copyFileAtomically(folder, "song.wav", { ByteArrayInputStream(ByteArray(200_000)) }, {
                    if (++checks == 2) throw CancellationException("left screen")
                })
                fail("copy should cancel")
            } catch (_: CancellationException) { }
            assertTrue(folder.list()!!.isEmpty())
        } finally { folder.deleteRecursively() }
    }

    @Test fun operationsFinishAfterSuccessFailureAndCancellation() = runBlocking {
        var completed = 0
        var succeeded = 0
        var error: Throwable? = null
        performOperation({ 42 }, { succeeded = it }, { error = it }, { completed++ })
        assertEquals(42, succeeded)
        performOperation<Int>({ throw IOException("offline") }, { fail("unexpected success") }, { error = it }, { completed++ })
        assertEquals("offline", error!!.message)
        try {
            performOperation<Int>({ throw CancellationException("cancelled") }, { fail() }, { fail("cancellation must propagate") }, { completed++ })
            fail("cancellation must propagate")
        } catch (_: CancellationException) { }
        assertEquals(3, completed)
    }
}
