#ifndef __FFMPEG_H__
#define __FFMPEG_H__

#ifdef LIBAVCODEC
#include "libavcodec/avcodec.h"
#endif

#ifdef LIBAVDEVICE
#include "libavdevice/avdevice.h"
#endif

#ifdef LIBAVFILTER
#include "libavfilter/avfilter.h"
#endif

#ifdef LIBAVFORMAT
#include "libavformat/avformat.h"
#endif

#ifdef LIBAVUTIL
#include "libavutil/avutil.h"
#endif

#ifdef LIBPOSTPROC
#include "libpostproc/postprocess.h"
#endif

#ifdef LIBSWRESAMPLE
#include "libswresample/swresample.h"
#endif

#ifdef LIBSWSCALE
#include "libswscale/swscale.h"
#endif

#endif  /* __FFMPEG_H__ */
