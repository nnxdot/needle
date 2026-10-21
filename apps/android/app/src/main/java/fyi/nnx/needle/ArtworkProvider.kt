package fyi.nnx.needle

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import java.io.File
import java.io.FileNotFoundException
import kotlinx.coroutines.launch

/**
 * Lets the lock screen, the notification, and Android Auto show covers. They run as other
 * apps and cannot open Needle's own files, so covers are handed out through this: read-only,
 * and only the cover pictures in Needle's artwork folder, nothing else.
 */
class ArtworkProvider : ContentProvider() {
    override fun onCreate(): Boolean {
        // A copy cut off by a closed app (see [shrink]) is never used: it goes.
        runCatching { folder().listFiles { f -> f.name.startsWith(".folder-") && f.name.endsWith(".part") }?.forEach { it.delete() } }
        return true
    }

    private fun folder() = File(context!!.filesDir, "needle/artwork").canonicalFile

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        if (mode != "r") throw SecurityException("Covers are read-only")
        val name = uri.lastPathSegment ?: throw FileNotFoundException()
        val file = File(folder(), name).canonicalFile
        // Only a file directly in the artwork folder: no "..", no other folders.
        if (file.parentFile != folder() || !file.isFile) throw FileNotFoundException(name)
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun getType(uri: Uri) = "image/*"
    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, args: Array<out String>?, sort: String?): Cursor? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, selection: String?, args: Array<out String>?) = 0
    override fun update(uri: Uri, values: ContentValues?, selection: String?, args: Array<out String>?) = 0

    companion object {
        /** Covers beside the songs being copied now, so each is copied once. */
        private val copying = java.util.Collections.synchronizedSet(HashSet<String>())
        private val failed = java.util.Collections.synchronizedSet(HashSet<String>())

        /** One copy at a time: a car's list of a thousand albums would decode them all at once. */
        private val copier = kotlinx.coroutines.Dispatchers.IO.limitedParallelism(1)

        /**
         * A cover others can open. One beside the songs (an album folder's cover.jpg) is outside
         * Needle's own files, so a small copy of it is made in the artwork folder, off the main
         * thread: until it is made (the first time it is asked for), there is none.
         */
        fun uri(path: String?): Uri? {
            path ?: return null
            val file = File(path)
            val app = NeedleApp.instance
            val folderFile = File(app.filesDir, "needle/artwork")
            val folder = folderFile.canonicalPath
            if (file.canonicalFile.parent == folder) return Uri.parse("content://${app.packageName}.artwork/${file.name}")
            if (!file.isFile) return null
            // Named by the file and its last change, so a new cover.jpg makes a new copy.
            val digest = java.security.MessageDigest.getInstance("SHA-1")
                .digest("$path|${file.lastModified()}|${file.length()}".toByteArray())
                .joinToString("") { "%02x".format(it) }
            val copy = File(folderFile, "folder-$digest.jpg")
            if (copy.isFile) return Uri.parse("content://${app.packageName}.artwork/${copy.name}")
            // A cover that could not be read is not tried again this run, at each list.
            if (copy.name !in failed && copying.add(copy.name)) {
                app.scope.launch(copier) {
                    if (runCatching { shrink(file, copy) }.getOrDefault(false) != true) failed += copy.name
                    copying.remove(copy.name)
                }
            }
            return null
        }

        /** A copy of a cover at most about 512 pixels across, written whole or not at all. */
        private fun shrink(from: File, to: File): Boolean {
            val bounds = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
            android.graphics.BitmapFactory.decodeFile(from.path, bounds)
            var sample = 1
            while (maxOf(bounds.outWidth, bounds.outHeight) / (sample * 2) >= 512) sample *= 2
            val bitmap = android.graphics.BitmapFactory.decodeFile(from.path, android.graphics.BitmapFactory.Options().apply { inSampleSize = sample })
                ?: return false
            to.parentFile?.mkdirs()
            val part = File(to.parentFile, ".${to.name}.part")
            val written = part.outputStream().use { bitmap.compress(android.graphics.Bitmap.CompressFormat.JPEG, 90, it) }
            bitmap.recycle()
            if (written && part.renameTo(to)) return true
            part.delete()
            return false
        }
    }
}
