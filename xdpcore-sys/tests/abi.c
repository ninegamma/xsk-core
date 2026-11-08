/* SPDX-License-Identifier: BSD-2-Clause */
#include <stddef.h>
#include <stdio.h>
#include <xdp/xsk.h>

#define TYPE(name) \
    printf(#name ".size=%zu\n", sizeof(struct name)); \
    printf(#name ".align=%zu\n", _Alignof(struct name))
#define FIELD(name, field) \
    printf(#name "." #field "=%zu\n", offsetof(struct name, field))
#define VALUE(name) printf(#name "=%llu\n", (unsigned long long)(name))
#define RING(name) \
    TYPE(name); \
    FIELD(name, cached_prod); FIELD(name, cached_cons); \
    FIELD(name, mask); FIELD(name, size); \
    FIELD(name, producer); FIELD(name, consumer); \
    FIELD(name, ring); FIELD(name, flags)

int main(void)
{
    RING(xsk_ring_prod);
    RING(xsk_ring_cons);
    TYPE(xdp_desc);
    FIELD(xdp_desc, addr); FIELD(xdp_desc, len); FIELD(xdp_desc, options);
    TYPE(xsk_umem_config);
    FIELD(xsk_umem_config, fill_size); FIELD(xsk_umem_config, comp_size);
    FIELD(xsk_umem_config, frame_size); FIELD(xsk_umem_config, frame_headroom);
    FIELD(xsk_umem_config, flags);
    TYPE(xsk_umem_opts);
    FIELD(xsk_umem_opts, sz); FIELD(xsk_umem_opts, fd);
    FIELD(xsk_umem_opts, size); FIELD(xsk_umem_opts, fill_size);
    FIELD(xsk_umem_opts, comp_size); FIELD(xsk_umem_opts, frame_size);
    FIELD(xsk_umem_opts, frame_headroom); FIELD(xsk_umem_opts, flags);
    FIELD(xsk_umem_opts, tx_metadata_len);
    TYPE(xsk_socket_config);
    FIELD(xsk_socket_config, rx_size); FIELD(xsk_socket_config, tx_size);
    printf("xsk_socket_config.__bindgen_anon_1=%zu\n",
           offsetof(struct xsk_socket_config, libxdp_flags));
    FIELD(xsk_socket_config, xdp_flags); FIELD(xsk_socket_config, bind_flags);
    TYPE(xdp_statistics);
    FIELD(xdp_statistics, rx_dropped); FIELD(xdp_statistics, rx_invalid_descs);
    FIELD(xdp_statistics, tx_invalid_descs); FIELD(xdp_statistics, rx_ring_full);
    FIELD(xdp_statistics, rx_fill_ring_empty_descs);
    FIELD(xdp_statistics, tx_ring_empty_descs);
    VALUE(XDP_PACKET_HEADROOM); VALUE(XDP_USE_NEED_WAKEUP);
    VALUE(XDP_RING_NEED_WAKEUP); VALUE(XDP_STATISTICS);
    VALUE(XSK_RING_CONS__DEFAULT_NUM_DESCS);
    VALUE(XSK_RING_PROD__DEFAULT_NUM_DESCS);
    VALUE(XSK_UMEM__DEFAULT_FLAGS);
    VALUE(XSK_UMEM__DEFAULT_FRAME_HEADROOM);
    VALUE(XSK_UMEM__DEFAULT_FRAME_SIZE);
    VALUE(XSK_UMEM__DEFAULT_TX_METADATA_LEN);
    return 0;
}
