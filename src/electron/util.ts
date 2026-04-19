import { ipcMain, WebContents, WebFrameMain } from "electron"
import { getUIPath } from "./pathResolver.js"
import { pathToFileURL } from "url"
import dotenv from "dotenv"

dotenv.config()
const PORT = process.env.PORT || "5173"
const isDevValue = process.env.NODE_ENV == "development"

export function isDev(): boolean {
   return isDevValue
}

// Making IPC Typesafe
export function ipcMainHandle<Key extends keyof EventPayloadMapping>(
   key: Key,
   handler: () => EventPayloadMapping[Key]
) {
   ipcMain.handle(key, event => {
      if (event.senderFrame) validateEventFrame(event.senderFrame)

      return handler()
   })
}

export function ipcWebContentsSend<Key extends keyof EventPayloadMapping>(
   key: Key,
   webContents: WebContents,
   payload: EventPayloadMapping[Key]
) {
   webContents.send(key, payload)
}

export function validateEventFrame(frame: WebFrameMain) {
   if (isDev() && new URL(frame.url).host === `localhost:${PORT}`) return

   if (frame.url !== pathToFileURL(getUIPath()).toString()) throw new Error("Malicious event")
}
