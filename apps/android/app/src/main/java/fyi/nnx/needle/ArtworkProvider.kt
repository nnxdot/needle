package fyi.nnx.needle

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import java.io.File
import java.io.FileNotFoundException

/**
 * Lets the lock screen, the notification, and Android Auto show covers. They run as other
 * apps and cannot open Needle's own files, so covers are handed out through this: read-only,
 * and only the cover pictures in Needle's artwork folder, nothing else.
 */
class ArtworkProvider : ContentProvider() {
    override fun onCreate() = true

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
        /** A cover others can open, or `null` for one outside the artwork folder. */
        fun uri(path: String?): Uri? {
            path ?: return null
            val file = File(path)
            val app = NeedleApp.instance
            val folder = File(app.filesDir, "needle/artwork").canonicalPath
            if (file.canonicalFile.parent != folder) return null
            return Uri.parse("content://${app.packageName}.artwork/${file.name}")
        }
    }
}
