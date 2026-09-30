export interface MemoryResolveCapabilitiesRequest {
  targetType: 'subject' | 'space' | 'binding' | 'memory';
  targetId: string;
  /** Opaque server-issued keyset cursor (pagination mode `cursor`, PAGINATION_SPEC section 3). Pass `data.pageInfo.nextCursor` from the previous page; omit for the first page. Client-constructed or forged cursors are rejected as invalid parameters. */
  cursor?: string;
  /** Page size for this cursor-mode listing: default 20, maximum 200. Page boundaries are stable keyset positions, not ordinal page numbers. */
  pageSize?: number;
}
