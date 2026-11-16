import type { CustomResponse, NewCustomResponse } from '@/features/custom-responses';

export interface LineDraft {
  key: string;
  text: string;
}

export interface ResponseDraft {
  triggers: LineDraft[];
  answers: LineDraft[];
}

const line = (text = ''): LineDraft => ({ key: crypto.randomUUID(), text });

export function responseDraft(response: CustomResponse | null): ResponseDraft {
  if (!response) return { triggers: [line()], answers: [line()] };
  return {
    triggers: response.triggers.map((text) => line(text)),
    answers: response.answers.map((text) => line(text)),
  };
}

export function emptyLine(): LineDraft {
  return line();
}

function filled(lines: LineDraft[]): string[] {
  return lines.map((l) => l.text.trim()).filter(Boolean);
}

export function readyToSave(draft: ResponseDraft): boolean {
  return filled(draft.triggers).length > 0 && filled(draft.answers).length > 0;
}

export function responseRequest(draft: ResponseDraft): NewCustomResponse {
  return { triggers: filled(draft.triggers), answers: filled(draft.answers) };
}
