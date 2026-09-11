import { useSyncExternalStore } from "react";
import type { Settings } from "./types";

export type Language = "en" | "de";
let language: Language = navigator.language.toLowerCase().startsWith("de")
  ? "de"
  : "en";
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};
const de: Record<string, string> = {
  Stop: "Stoppen",
  "Waiting for Windows permission": "Warte auf Windows-Zustimmung",
  "Resuming your task": "Deine Aufgabe wird fortgesetzt",
  "Your task continues after Windows permission is approved.":
    "Nach deiner Windows-Zustimmung geht deine Aufgabe weiter.",
  "Use Stop to cancel continuation.":
    "Mit Stoppen kannst du die Fortsetzung abbrechen.",
  "Windows permission is needed to control this app. Approve the Windows prompt to continue the task.":
    "Zum Steuern dieser App benötigt klickwerk Windows-Rechte. Bestätige die Windows-Abfrage, um die Aufgabe fortzusetzen.",
  "Administrator access was approved. Resuming your task from the current desktop…":
    "Die Administratorrechte wurden bestätigt. Deine Aufgabe wird auf dem aktuellen Desktop fortgesetzt…",
  "Automatic continuation was cancelled. Your task is paused.":
    "Die automatische Fortsetzung wurde abgebrochen. Deine Aufgabe ist pausiert.",
  "The restored task is preparing to continue. Stop it before starting another task.":
    "Die wiederhergestellte Aufgabe wird fortgesetzt. Stoppe sie, bevor du eine andere Aufgabe startest.",
  "Windows blocks input because this app has higher privileges than klickwerk. Restart klickwerk as administrator, or reopen the target app without administrator rights. A manual text entry will not unblock subsequent clicks.":
    "Windows blockiert Eingaben, weil diese App höhere Rechte als klickwerk hat. Starte klickwerk als Administrator neu oder öffne die Ziel-App ohne Administratorrechte. Eine manuelle Texteingabe hebt die Sperre für spätere Klicks nicht auf.",
  "Administrator rights needed": "Administratorrechte erforderlich",
  "Restart as administrator": "Als Administrator neu starten",
  "Windows will ask for permission. Your task and history are kept. Click Continue after the restart.":
    "Windows fragt nach deiner Zustimmung. Aufgabe und Verlauf bleiben erhalten. Klicke nach dem Neustart auf Weiter.",
  "klickwerk restarted as administrator. Click Continue to inspect the current desktop and resume your task.":
    "klickwerk wurde als Administrator neu gestartet. Klicke auf Weiter, um den aktuellen Desktop zu prüfen und die Aufgabe fortzusetzen.",
  "The administrator restart failed. Your session is still open. You can retry or export its history.":
    "Der Administrator-Neustart ist fehlgeschlagen. Deine Sitzung ist weiterhin geöffnet. Du kannst es erneut versuchen oder den Verlauf exportieren.",
  "The administrator restart was cancelled. Your session is still open.":
    "Der Administrator-Neustart wurde abgebrochen. Deine Sitzung ist weiterhin geöffnet.",
  "This session is not available for an administrator restart.":
    "Für diese Sitzung ist kein Administrator-Neustart verfügbar.",
  "Finish the current activity before restarting klickwerk.":
    "Beende die aktuelle Aktivität, bevor du klickwerk neu startest.",
  "Administrator restart is available in the Windows app.":
    "Der Administrator-Neustart ist in der Windows-App verfügbar.",
  "Includes the full history, window details and recent screenshots.":
    "Enthält den gesamten Verlauf, Fensterdetails und die letzten Screenshots.",
  "Windows blocks input to this app because it has higher privileges than klickwerk. Complete this step manually, or reopen the target normally if possible, then continue. Do not change Windows security settings.":
    "Windows blockiert Eingaben in diese App, weil sie höhere Rechte als klickwerk hat. Führe den Schritt selbst aus oder öffne die Ziel-App nach Möglichkeit normal. Klicke danach auf Weiter. Ändere keine Windows-Sicherheitseinstellungen.",
  "The action points at klickwerk itself. Bring the intended app into view, then continue or correct the instructions.":
    "Die Aktion zielt auf klickwerk selbst. Bringe die gewünschte App ins Bild. Fahre dann fort oder korrigiere die Anweisung.",
  "Windows could not verify the target app's input permissions. Bring an accessible target window into view, or complete this step manually, then continue.":
    "Windows konnte die Eingaberechte der Ziel-App nicht prüfen. Bringe ein zugängliches Zielfenster ins Bild oder führe den Schritt selbst aus. Fahre danach fort.",
  "The focused window or text field changed during input. Some text may already have arrived. Check it before continuing.":
    "Während der Eingabe hat sich das aktive Fenster oder Textfeld geändert. Ein Teil des Textes kann bereits angekommen sein. Prüfe ihn, bevor du fortfährst.",
  "Windows accepted only part of the input, or none. Input has been released. Check the target before continuing.":
    "Windows hat die Eingabe nur teilweise oder gar nicht angenommen. Alle Tasten sind losgelassen. Prüfe die Ziel-App, bevor du fortfährst.",
  "Windows switched to another input desktop. Return to the normal desktop, then continue.":
    "Windows hat zu einem anderen Eingabedesktop gewechselt. Kehre zum normalen Desktop zurück und fahre dann fort.",
  "The display layout changed. Observe the desktop again before continuing.":
    "Die Bildschirmanordnung hat sich geändert. Prüfe den Desktop, bevor du fortfährst.",
  "The foreground window changed. Observe again before sending input.":
    "Das aktive Fenster hat sich geändert. Prüfe das Zielfenster, bevor du fortfährst.",
  "The pointer target is outside a physical monitor.":
    "Das Mausziel liegt außerhalb eines angeschlossenen Bildschirms.",
  "The pointer target is outside the captured image.":
    "Das Mausziel liegt außerhalb des aufgenommenen Bildes.",
  "Export history (JSON)": "Verlauf exportieren (JSON)",
  "Exporting history…": "Verlauf wird exportiert…",
  "History exported to {name}.": "Verlauf nach {name} exportiert.",
  "This session is no longer available to export.":
    "Diese Sitzung ist nicht mehr zum Exportieren verfügbar.",
  "Pause the task before exporting its history.":
    "Pausiere die Aufgabe, bevor du den Verlauf exportierst.",
  "The refinement is too long to export.":
    "Die Verfeinerung ist zu lang für den Export.",
  "The history could not be saved. Check the folder permissions and available space.":
    "Der Verlauf konnte nicht gespeichert werden. Prüfe die Ordnerberechtigungen und den freien Speicherplatz.",
  "The history file could not be replaced. Choose another file or check its permissions.":
    "Die Verlaufsdatei konnte nicht ersetzt werden. Wähle eine andere Datei oder prüfe ihre Berechtigungen.",
  "The export dialog could not be opened.":
    "Der Exportdialog konnte nicht geöffnet werden.",
  "The export dialog could not be opened. Try again.":
    "Der Exportdialog konnte nicht geöffnet werden. Versuche es erneut.",
  "The export worker stopped unexpectedly.":
    "Der Export wurde unerwartet beendet.",
  "The session could not be encoded as JSON.":
    "Die Sitzung konnte nicht in JSON umgewandelt werden.",
  "Choose a folder for the export.": "Wähle einen Ordner für den Export.",
  "The main window is unavailable.": "Das Hauptfenster ist nicht verfügbar.",
  "Selection options": "Auswahlmöglichkeiten",
  "Make it yours. Changes save automatically.":
    "Ganz nach deinen Wünschen. Änderungen werden automatisch gespeichert.",
  "Close settings": "Einstellungen schließen",
  "Available models": "Verfügbare Modelle",
  "Show models": "Modelle anzeigen",
  "Loading models…": "Modelle werden geladen…",
  "No matching models. You can enter an exact model ID.":
    "Keine passenden Modelle. Du kannst eine genaue Modell-ID eingeben.",
  "Follows your desktop language": "Übernimmt die Sprache deines Desktops",
  "Follows your desktop appearance":
    "Übernimmt die Darstellung deines Desktops",
  "Smaller images, faster responses": "Kleinere Bilder, schnellere Antworten",
  "Best for most tasks": "Passend für die meisten Aufgaben",
  "Sharper text and small controls":
    "Schärfere Texte und kleine Bedienelemente",
  "Changes save automatically": "Änderungen werden automatisch gespeichert",
  "All changes saved": "Alle Änderungen gespeichert",
  "Changes not saved.": "Änderungen nicht gespeichert.",
  "Discard unsaved changes": "Ungespeicherte Änderungen verwerfen",
  "Enter a valid server address.": "Gib eine gültige Server-Adresse ein.",
  Done: "Fertig",
  "{count} seconds": "{count} Sekunden",
  "{count} step": "{count} Schritt",
  "{count} steps": "{count} Schritte",

  "Learn & use": "Lernen & verwenden",
  "My workflow": "Mein Workflow",
  "My desktop workflow": "Mein Desktop-Workflow",
  "Locate targets on the current desktop and verify each result before continuing.":
    "Finde die Ziele auf dem aktuellen Desktop und prüfe jedes Ergebnis, bevor du fortfährst.",
  "The previous attempt was not verified as complete; check the last attempted action before repeating it.":
    "Der vorherige Versuch wurde nicht als abgeschlossen bestätigt. Prüfe die letzte versuchte Aktion, bevor du sie wiederholst.",
  "The model server took too long. Check the server or increase the timeout in Settings.":
    "Der Modellserver hat zu lange gebraucht. Prüfe den Server oder erhöhe das Antwortzeitlimit in den Einstellungen.",
  "Cannot reach the model server. Check that it is running and the URL is correct.":
    "Der Modellserver ist nicht erreichbar. Prüfe, ob er läuft und die URL stimmt.",
  "The connection failed. Check the server and TLS certificate.":
    "Die Verbindung ist fehlgeschlagen. Prüfe den Server und das TLS-Zertifikat.",
  "The server refused access. Check the API key and permissions.":
    "Der Server verweigert den Zugriff. Prüfe den API-Schlüssel und die Berechtigungen.",
  "This API address or model was not found. Check Settings.":
    "Diese API-Adresse oder dieses Modell wurde nicht gefunden. Prüfe die Einstellungen.",
  "The server is busy or its usage limit was reached. Try again later.":
    "Der Server ist ausgelastet oder sein Nutzungslimit ist erreicht. Versuche es später erneut.",
  "The server returned a redirect. Enter its final API URL in Settings.":
    "Der Server antwortet mit einer Weiterleitung. Trage die endgültige API-URL in den Einstellungen ein.",
  "The server response was interrupted.":
    "Die Serverantwort wurde unterbrochen.",
  "The server did not return valid JSON.":
    "Der Server hat kein gültiges JSON geliefert.",
  "The model server returned no answer.":
    "Der Modellserver hat nicht geantwortet.",
  "The model returned no text. Select a compatible model.":
    "Das Modell hat keinen Text geliefert. Wähle ein kompatibles Modell.",
  "Connected. The model passed the image and action-format check.":
    "Verbunden. Das Modell hat den Test für Bildverständnis und Aktionsformat bestanden.",
  "The server responded, but the model did not locate the test shape. Select a compatible vision model.":
    "Der Server hat geantwortet, aber das Modell hat die Testform nicht gefunden. Wähle ein kompatibles Vision-Modell.",
  "Choose a vision model in Settings first.":
    "Wähle zuerst ein Vision-Modell in den Einstellungen.",
  "The screen has not changed after several steps. Bring the right window into view, then tell me how to continue.":
    "Der Bildschirm hat sich seit mehreren Schritten nicht verändert. Öffne das richtige Fenster und sag mir, wie es weitergeht.",
  "The target keeps changing. Bring it into view and tell me when to continue.":
    "Das Ziel verändert sich weiterhin. Bringe es in Sicht und sag mir, wann es weitergehen kann.",
  "Back to you.": "Du bist wieder dran.",
  "Review the result, refine it, or keep it for next time.":
    "Prüfe das Ergebnis, verfeinere es oder speichere es fürs nächste Mal.",
  "Saved start prompt": "Gespeicherter Startprompt",
  "Save current prompt": "Aktuellen Prompt speichern",
  "Learning is unavailable. Retry, or keep your prompt and corrections without consolidation.":
    "Das Lernen ist gerade nicht verfügbar. Versuche es erneut oder speichere deinen Prompt samt Korrekturen ohne Zusammenfassung.",
  "“{name}” is saved with your corrections. Learning was unavailable.":
    "„{name}“ ist mit deinen Korrekturen gespeichert. Das Lernen war nicht verfügbar.",
  "A few things to try": "Ein paar Ideen zum Ausprobieren",
  "A little help,": "Ein bisschen Hilfe, ",
  "A little less busywork.": "Weniger Kleinkram. Mehr Freiraum.",
  "A task worth repeating? Refine it as you go, then save it here.":
    "Eine Aufgabe für öfter? Verfeinere sie unterwegs und speichere sie hier.",
  "API key": "API-Schlüssel",
  "Action history": "Aktionsverlauf",
  Agent: "Agent",
  "Answer below to continue, or save this workflow for later.":
    "Antworte unten zum Fortfahren oder speichere den Workflow für später.",
  Appearance: "Darstellung",
  Automatic: "Automatisch",
  Balanced: "Ausgewogen",
  "Better with every refinement": "Mit jeder Verfeinerung besser",
  "Browser preview": "Browser-Vorschau",
  "Browser preview · all desktop actions are simulated":
    "Browser-Vorschau · alle Desktop-Aktionen sind simuliert",
  "Browser preview: learning is simulated. Only the start prompt is saved.":
    "Browser-Vorschau: Das Lernen ist simuliert. Gespeichert wird nur der Startprompt.",
  Cancel: "Abbrechen",
  "Check connection": "Verbindung prüfen",
  "Checking image understanding…": "Bildverständnis wird geprüft…",
  "Choose a model that can understand images. Exact model IDs also work.":
    "Wähle ein Modell mit Bildverständnis. Du kannst auch die genaue Modell-ID eingeben.",
  "Choose or enter a model ID": "Modell auswählen oder ID eingeben",
  "Combining useful steps, corrections and lessons from mistakes into one better starting point.":
    "Nützliche Schritte, Korrekturen und Erfahrungen aus Fehlern werden zu einem besseren Startprompt zusammengefasst.",
  Connection: "Verbindung",
  Continue: "Weiter",
  "Continue as is, or add a correction below. Nothing runs until you choose.":
    "Mach unverändert weiter oder ergänze unten eine Korrektur. Erst mit deiner Entscheidung geht es weiter.",
  "Couldn't open klickwerk": "klickwerk konnte nicht geöffnet werden",
  "Couldn’t finish. Back to you.": "Nicht abgeschlossen. Du bist wieder dran.",
  "Ctrl + Enter to": "Strg + Eingabe zum",
  Dark: "Dunkel",
  Delete: "Löschen",
  "Delete this workflow and start a fresh session?":
    "Diesen Workflow löschen und eine neue Sitzung starten?",
  "Delete workflow": "Workflow löschen",
  "Delete {name}": "{name} löschen",
  "Describe a task, just as you’d ask a person…":
    "Beschreibe eine Aufgabe, so wie du eine Person fragen würdest…",
  "Describe the outcome. I’ll handle the clicks and typing.":
    "Beschreibe dein Ziel. Ich kümmere mich ums Klicken und Tippen.",
  "Desktop:": "Desktop:",
  "Detach workflow": "Workflow lösen",
  Detailed: "Detailliert",
  Deutsch: "Deutsch",
  "Dictate your task": "Aufgabe diktieren",
  "Dismiss message": "Meldung schließen",
  "Done. Back to you.": "Erledigt. Du bist wieder dran.",
  "Edit the goal and preferences. Saving consolidates everything into this prompt.":
    "Passe Ziel und Wünsche an. Beim Speichern wird alles in diesem Prompt zusammengeführt.",
  English: "English",
  "Enter your server URL, add a key if needed, then choose a vision model.":
    "Server-URL eingeben, bei Bedarf den Schlüssel ergänzen, dann ein Vision-Modell auswählen.",
  Fast: "Schnell",
  "Finish dictation": "Diktat beenden",
  "Getting ready": "Vorbereitung läuft",
  "Getting ready…": "Wird vorbereitet…",
  "Getting ready to help…": "Ich bereite alles vor…",
  "Hide API key": "API-Schlüssel verbergen",
  "Hide actions": "Aktionen ausblenden",
  "In progress": "In Arbeit",
  "Input sent. The next screen check verifies the outcome.":
    "Eingabe gesendet. Die nächste Bildschirmprüfung kontrolliert das Ergebnis.",
  Language: "Sprache",
  "Learn & save": "Lernen & speichern",
  "Learn & update": "Lernen & aktualisieren",
  "Learn for next time": "Fürs nächste Mal lernen",
  "Learned start prompt": "Angelernter Startprompt",
  "Learning from this session…":
    "Erfahrungen aus dieser Sitzung werden eingearbeitet…",
  "Less clicking. More living.": "Weniger klicken. Mehr erleben.",
  "Let’s do it": "Los geht’s",
  "Let’s get it": "Machen wir es ",
  Light: "Hell",
  "Listening… Speak naturally, then stop the microphone.":
    "Ich höre zu… Sprich ganz normal und beende dann die Aufnahme.",
  "Load models": "Modelle laden",
  "Match your workspace.": "Passend zu deinem Arbeitsplatz.",
  "More detail sends larger images.": "Mehr Details bedeuten größere Bilder.",
  "Move your mouse or press any key to interrupt.":
    "Bewege die Maus oder drücke eine Taste zum Unterbrechen.",
  "Move your mouse or press any key to take over.":
    "Bewege die Maus oder drücke eine Taste zum Übernehmen.",
  "Move your mouse or type to take over. Anytime.":
    "Maus bewegen oder tippen zum Übernehmen. Jederzeit.",
  "Once running, move the mouse deliberately, click, or type to take over.":
    "Sobald der Agent arbeitet: Maus deutlich bewegen, klicken oder tippen, um zu übernehmen.",
  "Move the mouse deliberately, click, scroll, or press a key to interrupt.":
    "Zum Unterbrechen: Maus deutlich bewegen, klicken, scrollen oder eine Taste drücken.",
  "New task": "Neue Aufgabe",
  "No input sent. The target changed.":
    "Keine Eingabe gesendet. Das Ziel hat sich verändert.",
  "Nothing saved yet.": "Noch nichts gespeichert.",
  "One start prompt. Everything useful from this session built in.":
    "Ein Startprompt. Alles Nützliche aus dieser Sitzung steckt darin.",
  "Only if your server requires one": "Nur wenn dein Server einen benötigt",
  "Only the consolidated start prompt is saved. Action history stays in this session.":
    "Nur der zusammengeführte Startprompt wird gespeichert. Der Aktionsverlauf bleibt in dieser Sitzung.",
  "Open settings": "Einstellungen öffnen",
  "Opening workflow…": "Workflow wird geöffnet…",
  "Or try something new": "Oder probiere etwas Neues",
  "Paused. Back to you.": "Pausiert. Du bist wieder dran.",
  Preferences: "Allgemein",
  "Preview settings": "Vorschau-Einstellungen",
  "Recorded in the agent’s context.":
    "Im Sitzungskontext des Agenten vermerkt.",
  "Refine & continue": "Verfeinern & weiter",
  "Refine the result below, save what you learned, or start a new task.":
    "Verfeinere unten das Ergebnis, speichere die Erfahrungen oder starte eine neue Aufgabe.",
  "Refine your task": "Aufgabe verfeinern",
  "Remove saved key": "Gespeicherten Schlüssel entfernen",
  "Response timeout": "Antwortzeitlimit",
  "Review your connection, then retry or refine the task.":
    "Prüfe die Verbindung und versuche es erneut oder verfeinere die Aufgabe.",
  "Save as workflow": "Als Workflow speichern",
  "Save settings": "Einstellungen speichern",
  "Saved beside the app in config.cfg":
    "In config.cfg neben der App gespeichert",
  "Saved securely on this Windows account":
    "Sicher für dieses Windows-Konto gespeichert",
  "Saving the improved start prompt…":
    "Verbesserter Startprompt wird gespeichert…",
  "Saving…": "Wird gespeichert…",
  "Screen detail": "Bildschirmdetails",
  "Screenshot:": "Bildschirmfoto:",
  Sent: "Gesendet",
  "Server URL": "Server-URL",
  Session: "Sitzung",
  Settings: "Einstellungen",
  "Settings sections": "Einstellungsbereiche",
  "Show API key": "API-Schlüssel anzeigen",
  "Show recent actions": "Letzte Aktionen anzeigen",
  "Show all {count} entries": "Alle {count} Einträge anzeigen",
  "Start prompt": "Startprompt",
  "Start task · Ctrl + Enter": "Aufgabe starten · Strg + Eingabe",
  "Starting in a moment": "Es geht gleich los",
  "Stop microphone": "Mikrofon stoppen",
  "Stops automatically after this many steps.":
    "Stoppt automatisch nach dieser Anzahl an Schritten.",
  System: "System",
  "Task limit": "Schrittlimit",
  "Test connection": "Verbindung testen",
  "This action may have been only partly sent.":
    "Diese Aktion wurde möglicherweise nur teilweise gesendet.",
  "Time allowed for each model response.":
    "Maximale Wartezeit pro Modellantwort.",
  "Try again": "Erneut versuchen",
  "Turn this session into a better start next time.":
    "Mach diese Sitzung zum besseren Start fürs nächste Mal.",
  "Update workflow": "Workflow aktualisieren",
  "Use workflow": "Workflow verwenden",
  "Use your desktop language or choose your own.":
    "Desktop-Sprache übernehmen oder selbst wählen.",
  "Uses a test image.": "Verwendet ein Testbild.",
  "View actions": "Aktionen anzeigen",
  "Vision model": "Vision-Modell",
  "What can I take": "Was kann ich dir",
  "What happened": "Was passiert ist",
  "What would you like me to do?": "Was soll ich für dich tun?",
  "What would you like to change?": "Was möchtest du ändern?",
  "Workflow name": "Workflow-Name",
  "Working on your desktop": "Ich arbeite auf deinem Desktop",
  "YOUR DESKTOP. A LITTLE LIGHTER.": "DEIN DESKTOP. EIN BISSCHEN LEICHTER.",
  "You took over. The agent is paused.":
    "Du hast übernommen. Der Agent pausiert.",
  "Your answer": "Deine Antwort",
  "Your answer is needed": "Deine Antwort ist gefragt",
  "Your connection": "Deine Verbindung",
  "Your desktop stays private during this check.":
    "Dein Desktop bleibt bei diesem Test privat.",
  "Your latest refinement is included":
    "Deine letzte Verfeinerung fließt mit ein",
  "Your next instruction makes this workflow better.":
    "Deine nächste Anweisung macht diesen Workflow besser.",
  "Your pace. Your control.": "Dein Tempo. Deine Kontrolle.",
  "Your refinement": "Deine Verfeinerung",
  "Your task": "Deine Aufgabe",
  "Your workflow": "Dein Workflow",
  "Your workflows": "Deine Workflows",
  continue: "Fortfahren",
  elapsed: "vergangen",
  entries: "Einträge",
  entry: "Eintrag",
  "in motion.": "in Bewegung.",
  "just right.": "passend.",
  klickwerk: "klickwerk",
  "klickwerk home": "klickwerk Startseite",
  "localhost:11434 or https://your-server/v1":
    "localhost:11434 oder https://dein-server/v1",
  "off your hands?": "abnehmen?",
  optional: "optional",
  px: "px",
  seconds: "Sekunden",
  start: "Starten",
  steps: "Schritte",
  "Close dialog": "Dialog schließen",
  "Connect a model": "Modell verbinden",
  "Workflow deleted. A fresh session is ready.":
    "Workflow gelöscht. Eine neue Sitzung ist bereit.",
  "“{name}” learned from this session. The improved start prompt is saved.":
    "Die Erfahrungen sind in „{name}“ eingeflossen. Der verbesserte Startprompt ist gespeichert.",
  "{count} models found{partial}. Choose one with vision support.":
    "{count} Modelle gefunden{partial}. Wähle eines mit Bildverständnis.",
  " (partial list)": " (unvollständige Liste)",
  "No models found. You can still enter an exact model ID.":
    "Keine Modelle gefunden. Du kannst die genaue Modell-ID trotzdem eingeben.",
  "Preview connection looks good. No server was contacted.":
    "Die Vorschau-Verbindung sieht gut aus. Es wurde kein Server kontaktiert.",
  "Starting in 2 seconds. Move your mouse or press any key to interrupt.":
    "Start in 2 Sekunden. Maus bewegen oder eine Taste drücken zum Unterbrechen.",
  "Starting in 2 seconds. Input interruption begins after the countdown.":
    "Start in 2 Sekunden. Maus und Tastatur unterbrechen erst nach dem Countdown.",
  "Use Stop to cancel startup.": "Mit Stoppen kannst du den Start abbrechen.",
  "Taking a look at your desktop…": "Ich schaue mir deinen Desktop an…",
  "Choosing the next action…": "Ich wähle den nächsten Schritt…",
  "Waiting for the desktop to settle…":
    "Ich warte, bis der Desktop bereit ist…",
  "The repeated input was not sent. Waiting for the app, then checking a fresh screenshot.":
    "Die wiederholte Eingabe wurde nicht gesendet. Ich warte auf die App und prüfe dann eine neue Bildschirmaufnahme.",
  "The last input still has no visible result after waiting and checking again. Check the app, then tell me how to continue.":
    "Auch nach Warten und erneuter Prüfung hat die letzte Eingabe kein sichtbares Ergebnis. Prüfe die App und sag mir, wie es weitergeht.",
  "Writing your document…": "Ich schreibe dein Dokument…",
  "Your task is complete.": "Deine Aufgabe ist abgeschlossen.",
  "Control is paused while you reply.":
    "Der Agent pausiert, während du antwortest.",
  "You took over. Tell me what to do differently, and I’ll use your correction when we continue.":
    "Du hast übernommen. Sag mir, was ich ändern soll, oder mach unverändert weiter.",
  "You took over. The agent is paused. Continue when ready, or describe what to change.":
    "Du hast übernommen. Der Agent pausiert. Mach weiter, wenn du bereit bist, oder beschreibe die Änderung.",
  "Preview complete. Your desktop has not been changed.":
    "Vorschau abgeschlossen. Dein Desktop wurde nicht verändert.",
  "Which folder should I use for the new document?":
    "In welchem Ordner soll das neue Dokument liegen?",
  "Cannot reach the model server. Check that it is running and try again.":
    "Der Modellserver ist nicht erreichbar. Prüfe, ob er läuft, und versuche es erneut.",
  "The screen is still changing. Wait for the app to finish loading, then tell me to continue.":
    "Der Bildschirm verändert sich noch. Warte, bis die App fertig geladen hat, und sag mir dann, dass es weitergehen kann.",
  "The last input has no visible result yet. I paused to avoid sending it twice. Check the app, then tell me how to continue.":
    "Die letzte Eingabe hat noch kein sichtbares Ergebnis. Ich pausiere, damit sie nicht doppelt gesendet wird. Prüfe die App und sag mir, wie es weitergeht.",
  "The task reached its step limit. Refine the instructions to continue.":
    "Das Schrittlimit ist erreicht. Verfeinere die Anweisung oder fahre fort.",
  "This workflow is no longer available.":
    "Dieser Workflow ist nicht mehr verfügbar.",
  "This session changed. Save again to include the latest changes.":
    "Die Sitzung wurde geändert. Speichere erneut, um die neuesten Änderungen einzubeziehen.",
  "The model did not return valid workflow instructions. Try again.":
    "Das Modell hat keine gültigen Workflow-Anweisungen geliefert. Versuche es erneut.",
  "Microphone dictation is available in the Windows app. You can type your task here.":
    "Diktieren ist in der Windows-App verfügbar. Hier kannst du deine Aufgabe eintippen.",
  "Use an HTTP(S) URL without credentials, query parameters, or fragments.":
    "Verwende eine HTTP(S)-URL ohne Zugangsdaten, Abfrageparameter oder Fragmente.",
  "Type text": "Text eingeben",
  "Check the screen": "Bildschirm prüfen",
  Click: "Klick",
  "Right click": "Rechtsklick",
  "Double click": "Doppelklick",
  Move: "Bewegen",
  Drag: "Ziehen",
  "Scroll up": "Nach oben scrollen",
  "Scroll down": "Nach unten scrollen",
  Wait: "Warten",
  interrupted: "unterbrochen",
  failed: "fehlgeschlagen",
  skipped: "übersprungen",
  recorded: "vermerkt",
};
export function t(
  key: string,
  values: Record<string, string | number> = {},
): string {
  const text = language === "de" ? (de[key] ?? key) : key;
  return text.replace(/\{(\w+)\}/g, (match, name: string) =>
    String(values[name] ?? match),
  );
}
export function setLanguage(
  preference: Settings["language"],
  desktopLanguage: Language,
) {
  const next = preference === "system" ? desktopLanguage : preference;
  document.documentElement.lang = next;
  if (next !== language) {
    language = next;
    listeners.forEach((listener) => listener());
  }
}
export function useI18n() {
  const current = useSyncExternalStore(subscribe, () => language);
  return { t, language: current };
}
