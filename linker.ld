SECTIONS {
  /DISCARD/ : {
    *(.eh_frame)
    *(.eh_frame_hdr)
  }
}
INSERT AFTER .text;
