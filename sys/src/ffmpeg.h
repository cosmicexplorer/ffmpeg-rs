#ifndef __FFMPEG_H__
#define __FFMPEG_H__

#ifdef LIBAVCODEC
#include "libavcodec/version.h"
#endif

#ifdef LIBAVDEVICE
#include "libavdevice/version.h"
#endif

#ifdef LIBAVFILTER
#include "libavfilter/version.h"
#endif

#ifdef LIBAVFORMAT
#include "libavformat/version.h"
#endif

#ifdef LIBAVUTIL
#include "libavutil/version.h"
#endif

#ifdef LIBPOSTPROC
#include "libpostproc/version.h"
#endif

#ifdef LIBSWRESAMPLE
#include "libswresample/swresample.h"
#endif

#ifdef LIBSWSCALE
#include "libswscale/version.h"
#endif

#endif  /* __FFMPEG_H__ */
