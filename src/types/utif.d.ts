/**
 * Minimal type declarations for the `utif` package.
 * (The library ships without TypeScript types.)
 */

declare module "utif" {
  export interface IFD {
    width: number;
    height: number;
    [key: string]: unknown;
  }

  /** Decode a TIFF ArrayBuffer into a list of IFDs (Image File Directories). */
  export function decode(buffer: ArrayBuffer | Uint8Array): IFD[];

  /** Decode pixel data for a specific IFD. Mutates `ifd` in place (adds pixel data). */
  export function decodeImage(
    buffer: ArrayBuffer | Uint8Array,
    ifd: IFD,
    ifds: IFD[]
  ): void;

  /** Convert a decoded IFD's pixel data to RGBA8 (Uint8Array). */
  export function toRGBA8(ifd: IFD): Uint8Array;

  const _default: {
    decode: typeof decode;
    decodeImage: typeof decodeImage;
    toRGBA8: typeof toRGBA8;
  };
  export default _default;
}
