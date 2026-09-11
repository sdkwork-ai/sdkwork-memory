export interface PageInfo {
    mode: 'offset' | 'cursor';
    page?: number;
    pageSize?: number;
    totalItems?: string;
    totalPages?: number;
    nextCursor?: string | null;
    hasMore?: boolean;
}
//# sourceMappingURL=page-info.d.ts.map