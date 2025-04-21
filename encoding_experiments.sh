#!/usr/bin/env bash

src="$1"
if [[ "${src}" == "" ]]; then
exit 1
fi

streamId="$2"
jobName="${streamId}"
outputPath="/tmp/out"
mkdir -p "${outputPath}"

function enc1080() {
FFREPORT="file=${outputPath}/${jobName}_ffmpeg.log:level=40" ffmpeg \
-y -threads 0 -hwaccel cuda -hwaccel_device 0 -re -stream_loop -1 -fflags genpts \
-i "${src}" \
-filter_complex "[0:v]fps=fps=30:start_time=0:round=near,hwupload_cuda=device=0,split=4[f30q5142][f30q5139][f30q5136][f30q5133];[f30q5142]scale_cuda=w=-4:h=360[q5142];[f30q5139]scale_cuda=w=-4:h=468[q5139];[f30q5136]scale_cuda=w=-4:h=720[q5136];[f30q5133]scale_cuda=w=-4:h=1080[q5133]" \

-f tee -preset:v:0 p2 -tune ull -minrate:v:0 4178k -maxrate:v:0 5431k -bufsize:v:0 8356k -map [q5133] -c:v:0 h264_nvenc -b:v:0 4050k -profile:v:0 high -g:v:0 60 -map 0:a -c:a:0 aac -b:a:0 128k -ac:a:0 2 -gpu 0 -zerolatency 1 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -sc_threshold 0 -b_strategy 0 -strict experimental \

-f tee -preset:v:1 p2 -tune ull -minrate:v:1 1928k -maxrate:v:1 2506k -bufsize:v:1 3856k -map [q5136] -c:v:1 h264_nvenc -b:v:1 1800k -profile:v:1 high -g:v:1 60 -map 0:a -c:a:1 aac -b:a:1 128k -ac:a:1 2 -gpu 0 -zerolatency 1 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -sc_threshold 0 -b_strategy 0 -strict experimental \

-f tee -preset:v:2 p2 -tune ull -minrate:v:2 896k -maxrate:v:2 1165k -bufsize:v:2 1792k -map [q5139] -c:v:2 h264_nvenc -b:v:2 800k -profile:v:2 high -g:v:2 60 -map 0:a -c:a:2 aac -b:a:2 96k -ac:a:2 2 -gpu 0 -zerolatency 1 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -sc_threshold 0 -b_strategy 0 -strict experimental \

-f tee -preset:v:3 p2 -tune ull -minrate:v:3 514k -maxrate:v:3 668k -bufsize:v:3 1028k -map [q5142] -c:v:3 h264_nvenc -b:v:3 450k -profile:v:3 baseline -g:v:3 60 -map 0:a -c:a:3 aac -b:a:3 64k -ac:a:3 2 -gpu 0 -zerolatency 1 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -sc_threshold 0 -b_strategy 0 -strict experimental \

-map 0:a -c:a:4 aac -b:a:4 128k -ac:a:4 2 \

"[onfail=abort:f=dash:select=\'v:0,v:1,v:2,v:3,a:4\':seg_duration=6:window_size=5:extra_window_size=10:remove_at_exit=1:media_seg_name='seg6000-part1000-qsid834-stream\$RepresentationID\$-163875486-\$Number%05d\$.\$ext\$':init_seg_name='init-stream\$RepresentationID\$-163875486.\$ext\$':adaptation_sets='id=0,streams=v id=1,streams=a':frag_duration=1:frag_type=duration:utc_timing_url='http\://localhost\:1480/time':ldash=1:lhls=1:hls_playlist=1:use_timeline=0:use_template=1:streaming=1:index_correction=1:target_latency=2:ignore_io_errors=1:format_options=\'movflags=cmaf\':master_m3u8_publish_rate=1]http://localhost:8888/${streamId}/index.mpd|[onfail=abort:f=hls:select=\'v:0,a:0,v:1,a:1,v:2,a:2,v:3,a:3\':hls_flags=+delete_segments+independent_segments+program_date_time:hls_segment_filename=\'http://localhost:8888/${streamId}/ed-s-streamer-gc147/seg6000-qsid834-media%v-163875486-%05d.ts\':hls_base_url=\'ed-s-streamer-gc147/\':var_stream_map=\'v:0,a:0 v:1,a:1 v:2,a:2 v:3,a:3\':hls_list_size=5:hls_time=6:master_pl_name=master_mpegts.m3u8:master_pl_publish_rate=1:ignore_io_errors=1]http://localhost:8888/${streamId}/media_mpegts_%v.m3u8"
}

function enc720() {
FFREPORT="file=${outputPath}/${jobName}_ffmpeg.log:level=40" ffmpeg \

-y -threads 0 -hwaccel cuda -hwaccel_device 0 -re -stream_loop -1 -fflags genpts \

-i "${src}" \

-filter_complex "[0:v]fps=fps=30:start_time=0:round=near,hwupload_cuda=device=0,split=3[f30q5142][f30q5139][f30q5136];[f30q5142]scale_cuda=w=-4:h=360[q5142];[f30q5139]scale_cuda=w=-4:h=468[q5139];[f30q5136]scale_cuda=w=-4:h=720[q5136]" \

-f tee -preset:v:0 p2 -tune ull -minrate:v:0 1928k -maxrate:v:0 2506k -bufsize:v:0 3856k -map [q5136] -c:v:0 h264_nvenc -b:v:0 1800k -profile:v:0 high -g:v:0 60 -map 0:a -c:a:0 aac -b:a:0 128k -ac:a:0 2 -gpu 0 -zerolatency 1 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -sc_threshold 0 -b_strategy 0 -strict experimental \

-f tee -preset:v:1 p2 -tune ull -minrate:v:1 896k -maxrate:v:1 1165k -bufsize:v:1 1792k -map [q5139] -c:v:1 h264_nvenc -b:v:1 800k -profile:v:1 high -g:v:1 60 -map 0:a -c:a:1 aac -b:a:1 96k -ac:a:1 2 -gpu 0 -zerolatency 1 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -sc_threshold 0 -b_strategy 0 -strict experimental \

-f tee -preset:v:2 p2 -tune ull -minrate:v:2 514k -maxrate:v:2 668k -bufsize:v:2 1028k -map [q5142] -c:v:2 h264_nvenc -b:v:2 450k -profile:v:2 baseline -g:v:2 60 -map 0:a -c:a:2 aac -b:a:2 64k -ac:a:2 2 -gpu 0 -zerolatency 1 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -sc_threshold 0 -b_strategy 0 -strict experimental \

-map 0:a -c:a:3 aac -b:a:3 128k -ac:a:3 2 \

"[onfail=abort:f=dash:select=\'v:0,v:1,v:2,a:3\':seg_duration=6:window_size=5:extra_window_size=10:remove_at_exit=1:media_seg_name='seg6000-part1000-qsid834-stream\$RepresentationID\$-163875486-\$Number%05d\$.\$ext\$':init_seg_name='init-stream\$RepresentationID\$-163875486.\$ext\$':adaptation_sets='id=0,streams=v id=1,streams=a':frag_duration=1:frag_type=duration:utc_timing_url='http\://localhost\:1480/time':ldash=1:lhls=1:hls_playlist=1:use_timeline=0:use_template=1:streaming=1:index_correction=1:target_latency=2:ignore_io_errors=1:format_options=\'movflags=cmaf\':master_m3u8_publish_rate=1]http://localhost:8888/${streamId}/index.mpd|[onfail=abort:f=hls:select=\'v:0,a:0,v:1,a:1,v:2,a:2\':hls_flags=+delete_segments+independent_segments+program_date_time:hls_segment_filename=\'http://localhost:8888/${streamId}/ed-s-streamer-gc147/seg6000-qsid834-media%v-163875486-%05d.ts\':hls_base_url=\'ed-s-streamer-gc147/\':var_stream_map=\'v:0,a:0 v:1,a:1 v:2,a:2\':hls_list_size=5:hls_time=6:master_pl_name=master_mpegts.m3u8:master_pl_publish_rate=1:ignore_io_errors=1]http://localhost:8888/${streamId}/media_mpegts_%v.m3u8"
}

function enc4k() {
FFREPORT="file=${outputPath}/${jobName}_ffmpeg.log:level=40" ffmpeg \

-y -threads 0 -hwaccel cuda -hwaccel_device 0 -re -stream_loop -1 -fflags genpts \

-i "${src}" \

-filter_complex "[0:v]format=yuv420p,setsar=sar=1[flt0];[flt0]fps=fps=30:start_time=0:round=near,hwupload_cuda=device=0,split=6[f30q4097][f30q4094][f30q4091][f30q4088][f30q5166][f30q5169];[f30q4097]scale_cuda=w=-4:h=360[q4097];[f30q4094]scale_cuda=w=-4:h=468[q4094];[f30q4091]scale_cuda=w=-4:h=720[q4091];[f30q4088]scale_cuda=w=-4:h=1080[q4088];[f30q5166]scale_cuda=w=-4:h=1440[q5166];[f30q5169]scale_cuda=w=-4:h=2160[q5169]" \

-f tee -preset:v:0 p2 -tune ull -minrate:v:0 4178k -maxrate:v:0 5431k -bufsize:v:0 8356k -map [q4088] -c:v:0 h264_nvenc -b:v:0 4050k -profile:v:0 high -g:v:0 60 -map 0:a -c:a:0 aac -b:a:0 128k -ac:a:0 2 -gpu 0 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -zerolatency:v 1 -tune:v ull -delay:v 0 -no-scenecut:v 1 -fps_mode:v passthrough -sc_threshold 0 -b_strategy 0 -strict experimental \

-f tee -preset:v:1 p2 -tune ull -minrate:v:1 1896k -maxrate:v:1 2465k -bufsize:v:1 3792k -map [q4091] -c:v:1 h264_nvenc -b:v:1 1800k -profile:v:1 high -g:v:1 60 -map 0:a -c:a:1 aac -b:a:1 96k -ac:a:1 2 -gpu 0 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -zerolatency:v 1 -tune:v ull -delay:v 0 -no-scenecut:v 1 -fps_mode:v passthrough -sc_threshold 0 -b_strategy 0 -strict experimental \

-f tee -preset:v:2 p2 -tune ull -minrate:v:2 896k -maxrate:v:2 1165k -bufsize:v:2 1792k -map [q4094] -c:v:2 h264_nvenc -b:v:2 800k -profile:v:2 high -g:v:2 60 -map 0:a -c:a:2 aac -b:a:2 96k -ac:a:2 2 -gpu 0 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -zerolatency:v 1 -tune:v ull -delay:v 0 -no-scenecut:v 1 -fps_mode:v passthrough -sc_threshold 0 -b_strategy 0 -strict experimental \

-f tee -preset:v:3 p2 -tune ull -minrate:v:3 514k -maxrate:v:3 668k -bufsize:v:3 1028k -map [q4097] -c:v:3 h264_nvenc -b:v:3 450k -profile:v:3 baseline -g:v:3 60 -map 0:a -c:a:3 aac -b:a:3 64k -ac:a:3 2 -gpu 0 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -zerolatency:v 1 -tune:v ull -delay:v 0 -no-scenecut:v 1 -fps_mode:v passthrough -sc_threshold 0 -b_strategy 0 -strict experimental \

-f tee -preset:v:4 p2 -tune ull -minrate:v:4 7392k -maxrate:v:4 9610k -bufsize:v:4 14784k -map [q5166] -c:v:4 h264_nvenc -b:v:4 7200k -profile:v:4 high -g:v:4 60 -map 0:a -c:a:4 aac -b:a:4 192k -ac:a:4 2 -gpu 0 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -zerolatency:v 1 -tune:v ull -delay:v 0 -no-scenecut:v 1 -fps_mode:v passthrough -sc_threshold 0 -b_strategy 0 -strict experimental \

-f tee -preset:v:5 p2 -tune ull -minrate:v:5 14192k -maxrate:v:5 18450k -bufsize:v:5 28384k -map [q5169] -c:v:5 h264_nvenc -b:v:5 14000k -profile:v:5 high -g:v:5 60 -map 0:a -c:a:5 aac -b:a:5 192k -ac:a:5 2 -gpu 0 -format_options movflags=cmaf -flags +global_header+cgop -fflags +nobuffer -max_muxing_queue_size 1000 -zerolatency:v 1 -tune:v
-delay:v 0 -no-scenecut:v 1 -fps_mode:v passthrough -sc_threshold 0 -b_strategy 0 -strict experimental \

-map 0:a -c:a:6 aac -b:a:6 128k -ac:a:6 2 \

"[onfail=abort:f=dash:select=\'v:0,v:1,v:2,v:3,v:4,v:5,a:6\':seg_duration=2:window_size=1800:extra_window_size=10:remove_at_exit=1:media_seg_name='seg2000-part500-qsid722-stream\$RepresentationID\$-163955014-\$Number%05d\$.\$ext\$':init_seg_name='init-stream\$RepresentationID\$-163955014.\$ext\$':adaptation_sets='id=0,streams=v id=1,streams=a':window_size=2:frag_duration=0.5:frag_type=duration:utc_timing_url='http\://localhost\:1480/time':ldash=1:lhls=1:hls_playlist=1:use_timeline=0:use_template=1:streaming=1:index_correction=1:target_latency=1.5:ignore_io_errors=1:format_options=\'movflags=cmaf\':master_m3u8_publish_rate=1:write_prft=1]http://localhost:8888/${streamId}/index.mpd|[onfail=abort:f=hls:select=\'v:0,a:0,v:1,a:1,v:2,a:2,v:3,a:3,v:4,a:4,v:5,a:5\':hls_flags=+delete_segments+independent_segments+program_date_time:hls_segment_filename=\'http://localhost:8888/${streamId}/am3-s-streamer-gc98/seg2000-qsid722-media%v-163955014-%05d.ts\':hls_base_url=\'am3-s-streamer-gc98/\':var_stream_map=\'v:0,a:0 v:1,a:1 v:2,a:2 v:3,a:3 v:4,a:4 v:5,a:5\':hls_list_size=1800:hls_time=2:master_pl_name=master_mpegts.m3u8:master_pl_publish_rate=1:ignore_io_errors=1]http://localhost:8888/${streamId}/media_mpegts_%v.m3u8"
}