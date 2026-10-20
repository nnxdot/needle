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

/**
 * A name short enough for the file system: names may be 255 bytes, and a long name in Chinese
 * or emoji is three or four bytes a letter. The end is kept (with the extension), and room is
 * left for the id put before it.
 */
private fun fitName(name: String, bytes: Int = 180): String {
    var kept = name
    while (kept.toByteArray(Charsets.UTF_8).size > bytes) kept = kept.drop(1)
    return kept
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
        .replace(Regex("[\\p{Cntrl}:*?\"<>|]"), "_").let(::fitName).ifBlank { "opened" }
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
