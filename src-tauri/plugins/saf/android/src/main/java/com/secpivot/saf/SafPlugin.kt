package com.secpivot.saf

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class OpenDocumentOptions {
  var mimeTypes: Array<String>? = null
}

@InvokeArg
class CreateDocumentOptions {
  var fileName: String? = null
  var mimeTypes: Array<String>? = null
}

@TauriPlugin
class SafPlugin(private val activity: Activity) : Plugin(activity) {
  private fun grantFlags(): Int =
    Intent.FLAG_GRANT_READ_URI_PERMISSION or
      Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
      Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION

  @Command
  fun openDocument(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(OpenDocumentOptions::class.java)
      val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
        addCategory(Intent.CATEGORY_OPENABLE)
        type = "*/*"
        args.mimeTypes?.let { putExtra(Intent.EXTRA_MIME_TYPES, it) }
        addFlags(grantFlags())
      }
      startActivityForResult(invoke, intent, "openDocumentResult")
    } catch (ex: Exception) {
      invoke.reject(ex.message ?: "无法打开文件选择器")
    }
  }

  @ActivityCallback
  fun openDocumentResult(invoke: Invoke, result: ActivityResult) {
    finishPick(invoke, result)
  }

  @Command
  fun createDocument(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(CreateDocumentOptions::class.java)
      val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
        addCategory(Intent.CATEGORY_OPENABLE)
        type = "*/*"
        args.mimeTypes?.let { putExtra(Intent.EXTRA_MIME_TYPES, it) }
        putExtra(Intent.EXTRA_TITLE, args.fileName ?: "vault.kdbx")
        addFlags(grantFlags())
      }
      startActivityForResult(invoke, intent, "createDocumentResult")
    } catch (ex: Exception) {
      invoke.reject(ex.message ?: "无法打开保存位置选择器")
    }
  }

  @ActivityCallback
  fun createDocumentResult(invoke: Invoke, result: ActivityResult) {
    finishPick(invoke, result)
  }

  private fun finishPick(invoke: Invoke, result: ActivityResult) {
    val data = result.data
    val uri = if (result.resultCode == Activity.RESULT_OK) data?.data else null
    if (uri == null) {
      invoke.resolve(JSObject())
      return
    }

    val takeFlags =
      data!!.flags and
        (Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
    if (takeFlags != 0) {
      try {
        activity.contentResolver.takePersistableUriPermission(uri, takeFlags)
      } catch (_: SecurityException) {
        // Some providers grant only a session-scoped URI; the open still works.
      }
    }

    val response = JSObject()
    response.put("uri", uri.toString())
    displayName(uri)?.let { response.put("name", it) }
    invoke.resolve(response)
  }

  private fun displayName(uri: Uri): String? {
    if (uri.scheme == "content") {
      try {
        activity.contentResolver
          .query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
          ?.use { cursor ->
            if (cursor.moveToFirst()) {
              val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
              if (index >= 0) return cursor.getString(index)
            }
          }
      } catch (_: Exception) {
        // Fall back to the URI's last path segment.
      }
    }
    return uri.lastPathSegment
  }
}
