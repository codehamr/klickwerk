import { CalendarDays, FileSearch, Sparkles } from "lucide-react";
import type { Language } from "./i18n";

export function taskSuggestions(language: Language) {
  return language === "de"
    ? [
        {
          icon: FileSearch,
          title: "Aus Tabs wird Klarheit",
          detail: "Offene Recherche in eine brauchbare Notiz verwandeln",
          prompt:
            "Sieh dir die aktuell geöffneten Recherche-Tabs in meinem Browser an. Erstelle in einem Texteditor eine kompakte Übersicht mit den wichtigsten Erkenntnissen, Quellenlinks und noch offenen Fragen. Markiere widersprüchliche Angaben und lass die Notiz zur Prüfung geöffnet.",
        },
        {
          icon: CalendarDays,
          title: "Ein Wochenende nach Maß",
          detail: "Ideen mit Preisen, Wegen und einem Plan B",
          prompt:
            "Hilf mir, ein besonderes Wochenende zu planen. Frage zuerst nach Startort, Datum, Budget und Interessen. Recherchiere dann drei passende Ausflüge mit aktuellen Öffnungszeiten, Kosten, Anreise und einer Alternative bei Regen. Stelle die Optionen mit Quellenlinks in einer Notiz gegenüber und lass sie zur Auswahl geöffnet.",
        },
        {
          icon: Sparkles,
          title: "Von Stichpunkten zum Plan",
          detail: "Aus der offenen Notiz klare nächste Schritte machen",
          prompt:
            "Lies die Notiz, die gerade auf meinem Desktop geöffnet ist. Erstelle daneben in einem neuen Dokument einen klaren Aktionsplan mit Aufgaben, Reihenfolge, Abhängigkeiten und offenen Fragen. Übernimm Termine oder Zuständigkeiten nur, wenn sie in der Notiz stehen. Lass das Original unverändert und den Entwurf zur Prüfung geöffnet.",
        },
      ]
    : [
        {
          icon: FileSearch,
          title: "Make sense of your tabs",
          detail: "Turn open research into a useful brief",
          prompt:
            "Review the research tabs currently open in my browser. Create a concise brief in a text editor with the key findings, source links and unanswered questions. Flag conflicting claims and leave the brief open for me to review.",
        },
        {
          icon: CalendarDays,
          title: "A weekend worth planning",
          detail: "Ideas with costs, travel and a rainy-day backup",
          prompt:
            "Help me plan a memorable weekend. First ask for my starting location, dates, budget and interests. Then research three suitable outings with current opening hours, costs, travel options and a rainy-day alternative. Compare the options with source links in a note and leave it open for me to choose.",
        },
        {
          icon: Sparkles,
          title: "Turn notes into next steps",
          detail: "Give the open note a clear action plan",
          prompt:
            "Read the note currently open on my desktop. Create a clear action plan in a new document with tasks, sequence, dependencies and open questions. Include dates or owners only when they appear in the note. Keep the original unchanged and leave the draft open for review.",
        },
      ];
}
