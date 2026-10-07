package fyi.nnx.needle

import android.content.Intent
import android.database.sqlite.SQLiteDatabase
import android.net.Uri
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

@RunWith(AndroidJUnit4::class)
class OpenWithTest {
    @Test fun identicalProviderNamesKeepBothAudioFilesAndLibraryIdentities() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val target = instrumentation.targetContext
        val providerApp = instrumentation.context
        val folder = File(target.filesDir, "opened")
        val before = folder.listFiles().orEmpty().toSet()
        for ((index, name) in listOf("one", "two").withIndex()) {
            providerApp.startActivity(Intent(Intent.ACTION_VIEW).apply {
                setClassName(target.packageName, MainActivity::class.java.name)
                setDataAndType(Uri.parse("content://fyi.nnx.needle.test.audio/$name"), "audio/wav")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_GRANT_READ_URI_PERMISSION)
            })
            await { (folder.listFiles().orEmpty().toSet() - before).count { it.extension == "wav" } == index + 1 }
        }
        val copies = (folder.listFiles().orEmpty().toSet() - before).filter { it.extension == "wav" }
        assertEquals(2, copies.size)
        assertNotEquals(copies[0].name, copies[1].name)
        assertTrue(copies.any { it.readBytes().contentEquals(AuditAudioProvider.recording(440)) })
        assertTrue(copies.any { it.readBytes().contentEquals(AuditAudioProvider.recording(880)) })
        val db = File(target.filesDir, "needle/library.db")
        await {
            SQLiteDatabase.openDatabase(db.path, null, SQLiteDatabase.OPEN_READONLY).use { library ->
                // Rust canonicalizes paths; Android exposes aliases such as /data/data and /data/user/0.
                library.rawQuery("SELECT id, path FROM tracks WHERE path LIKE '%/opened/%'", null).use { rows ->
                    val ids = mutableSetOf<String>()
                    while (rows.moveToNext()) {
                        if (copies.any { it.name == File(rows.getString(1)).name }) ids.add(rows.getString(0))
                    }
                    if (ids.size == 2) assertTrue("App-owned copies need no shared-storage grant", NeedleApp.instance.core.songsArePrivate(ids.toList()))
                    ids.size == 2
                }
            }
        }
    }

    private fun await(condition: () -> Boolean) {
        val deadline = System.nanoTime() + 10_000_000_000L
        while (!condition()) {
            check(System.nanoTime() < deadline) { "Timed out waiting for provider import" }
            Thread.sleep(50)
        }
    }
}
