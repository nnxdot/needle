package fyi.nnx.needle

import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import java.nio.file.Files
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext

internal suspend fun copyFileOnIo(folder: File, displayName: String, open: () -> InputStream): File {
    var published: File? = null
    try {
        return withContext(Dispatchers.IO) {
            val task = currentCoroutineContext()
            copyFileAtomically(folder, displayName, open) { task.ensureActive() }.also { published = it }
        }
    } catch (error: Throwable) {
        published?.delete()
        throw error
    }
}

/** Each import is a new file; only a complete copy becomes visible. */
internal fun copyFileAtomically(
    folder: File,
    displayName: String,
    open: () -> InputStream,
    checkCancelled: () -> Unit = {},
): File {
    check(folder.isDirectory || folder.mkdirs()) { "Could not create the import folder" }
    val name = displayName.substringAfterLast('/').substringAfterLast('\\')
        .replace(Regex("[\\p{Cntrl}:*?\"<>|]"), "_").takeLast(160).ifBlank { "opened" }
    val file = File(folder, "${UUID.randomUUID()}-$name")
    val temporary = File.createTempFile(".copy-", ".part", folder)
    try {
        open().use { input ->
            FileOutputStream(temporary).use { output ->
                val buffer = ByteArray(64 * 1024)
                while (true) {
                    checkCancelled()
                    val n = input.read(buffer)
                    if (n < 0) break
                    output.write(buffer, 0, n)
                }
                output.fd.sync()
            }
        }
        checkCancelled()
        Files.move(temporary.toPath(), file.toPath())
        return file
    } finally {
        temporary.delete()
    }
}
