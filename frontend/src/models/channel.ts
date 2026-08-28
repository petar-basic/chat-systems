export interface CreateChannelDraft {
  name: string;
  description?: string;
  isPrivate: boolean;
  announcementOnly: boolean;
}
