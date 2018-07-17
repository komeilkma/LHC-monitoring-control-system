<?php
if (file_exists('install')) {
    echo "Click <a href='install/'>here</a> to proceed to the installer or else remove the install directory after installation and then refresh the page to continue."; 
    exit(); 
}
include("includes/controller.php");
$pagename = 'index';
$container = '';

if(!$session->isAdmin()){
    header("Location: login.php");
    exit;
}
else{
$total_users = $adminfunctions->totalUsers();
?>
<!DOCTYPE html>
<html>
    <head>
        <title>IPM Software</title>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">

        <link href="css/bootstrap.min.css" rel="stylesheet">
        <link href="fonts/Open Iconic/css/open-iconic-bootstrap.min.css" rel="stylesheet">
        <link href="fonts/font-awesome/css/font-awesome.min.css" rel="stylesheet">

        <link href="css/navigation.css" rel="stylesheet">
        <link href="css/style.css" rel="stylesheet">
        <link href="css/animation.css" rel="stylesheet">                
        
        <!-- Awesome Bootstrap Checkboxes CSS -->
        <link href="css/plugins/awesome-bootstrap-checkbox/awesome-bootstrap-checkbox.css" rel="stylesheet">      
       
    </head>
    <body>
        <!-- Page Wrapper -->
        <div id="page-wrapper">

            <!-- Side Menu -->
            <nav id="side-menu" class="navbar-default navbar-static-side" role="navigation">
                <div id="sidebar-collapse">
                    <div id="logo-element">
                        <a class="logo" href="index.php">
                          <img src="logo2.png">
                        </a>
                    </div>
                    <?php include('navigation.php'); ?>
                </div>
            </nav>
            <!-- END Side Menu -->

            <?php include('top-navbar.php'); ?>     

            <!-- Page Content -->
            <div id="page-content" class="gray-bg">
                
                <!-- Title Header -->
                <div class="title-header white-bg">
                    <h2>The Dashboard</h2>
                    <ol class="breadcrumb">
                        <li>
                            <a href="index.php">Home</a>
                        </li>
                        <li class="active">
                            The Dashboard
                        </li>
                    </ol>
                </div>
                <!-- END Title Header -->

                <div class="row">                                 
                    <div class="col-sm-6 col-md-3">
                            <div class="panel warm-blue-bg">
                                <div class="panel-body">
                                    <div class="icon-bg">
                                        <i class="oi oi-signal"></i>
                                    </div>
                                    <div class="text-center">
                                        <h4><?php echo $session->getNumMembers(); ?> Users</h4>
                                    </div>
                                </div>
                                <div class="panel-footer clearfix panel-footer-link ">
                                    Registered
                                </div>  
                            </div>
                        </div>
                        <div class="col-sm-6 col-md-3">
                            <div class="panel purple-bg">
                                <div class="panel-body">
                                    <div class="icon-bg">
                                        <i class="oi oi-dollar"></i>
                                    </div>
                                    <div class="text-center">
                                        <h4><?php echo $functions->calcNumActiveUsers(); ?> Users</h4>
                                    </div>
                                </div>
                                <div class="panel-footer clearfix panel-footer-link ">
                                    Currently Online
                                </div>  
                            </div>
                        </div>
                        <div class="col-sm-6 col-md-3">
                            <div class="panel red-bg">
                                <div class="panel-body">
                                    <div class="icon-bg">
                                        <i class="oi oi-envelope-closed"></i>
                                    </div>
                                    <div class="text-center">                                           
                                        <h4><?php echo $adminfunctions->usersSince($session->username); ?> New Users</h4>
                                    </div>
                                </div>
                                <div class="panel-footer clearfix panel-footer-link ">
                                    Since Last Visit
                                </div>                               
                            </div>
                        </div>
                        <div class="col-sm-6 col-md-3">
                            <div class="panel orange-bg">
                                <div class="panel-body">
                                    <div class="icon-bg">
                                        <i class="oi oi-clipboard"></i>
                                    </div>
                                    <div class="text-center">                                          
                                        <h4><?php echo $configs->getConfig('record_online_users'); ?> Users Online </h4>
                                    </div>
                                </div>
                                <div class="panel-footer clearfix panel-footer-link ">
                                    <?php echo date('M j, Y, g:i a', $configs->getConfig('record_online_date')); ?> 
                                </div>  
                            </div>
                        </div>
                </div>
                <!-- END Row -->
                
                <div class="row">                  
   
                    <div class="col-md-12 col-lg-6">                      
                            <div class="panel">
                                <!--Panel heading-->
                                <div class="panel-heading">
                                    <div class="panel-options pull-right">
                                        <button class="btn btn-sm expand-panel"><i class="oi oi-fullscreen-enter"></i></button>
                                        <button class="btn btn-sm close-panel"><i class="oi oi-x"></i></button>
                                        <button class="btn btn-sm minimise-panel"><i class="oi oi-minus"></i></button>
                                    </div>
                                    <i class="oi oi-folder"></i><h3 class="panel-title">Last 5 Visitors</h3>
                                </div>
                                <!--Panel body-->                          
                                <div class="panel-body table-responsive"> 
                                    <table class="table table-hover table-striped">
                                        <thead>
                                            <tr>
                                                <th>Username</th>
                                                <th>Status</th>
                                                <th>Last Visit</th>
                                                <th>Registered</th>
                                                <th class='text-center'>View</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            <?php
                                            $sql = "SELECT * FROM users WHERE username != '" . ADMIN_NAME . "' ORDER BY timestamp DESC LIMIT 5";
                                            $result = $db->prepare($sql);
                                            $result->execute();
                                            while ($row = $result->fetch()) {
                                                $reg = $adminfunctions->displayDate($row['regdate']);
                                                $lastlogin = $adminfunctions->displayDate($row['timestamp']);
                                                $email = $row['email'];
                                                $email = strlen($email) > 25 ? substr($email, 0, 25) . "..." : $email;

                                                echo "<tr>";
                                                echo "<td><a href='adminuseredit.php?usertoedit=" . $row['username'] . "'>" . wordwrap($row['username'],15,"<br>\n",TRUE) . "</a></td><td>" . $adminfunctions->displayStatus($row['username']) . "</td>";
                                                echo "<td>" . $lastlogin . "</td><td>" . $reg . "</td>";
                                                echo "<td class='text-center'><div class='btn-group btn-group-xs'><a href='adminuseredit.php?usertoedit=" . $row['username'] . "' title='Edit' class='open_modal btn btn-default'><i class='fa fa-pencil'></i> View</a></div></td>";
                                                echo "</tr>";
                                            }
                                            ?>
                                        </tbody>
                                    </table>
                                </div>
                            </div>

                            <div class="panel">
                                <!--Panel heading-->
                                <div class="panel-heading">
                                    <div class="panel-options pull-right">
                                        <button class="btn btn-sm expand-panel"><i class="oi oi-fullscreen-enter"></i></button>
                                        <button class="btn btn-sm close-panel"><i class="oi oi-x"></i></button>
                                        <button class="btn btn-sm minimise-panel"><i class="oi oi-minus"></i></button>
                                    </div>
                                    <i class="oi oi-folder"></i><h3 class="panel-title">Last 5 Registered Users</h3>
                                </div>
                                <!--Panel body-->                          
                                <div class="panel-body table-responsive"> 
                                    <table class="table table-hover table-striped">
                                        <thead>
                                            <tr>
                                                <th>Username</th>
                                                <th>Status</th>
                                                <th>Registered</th>
                                                <th>Last Visit</th>
                                                <th class='text-center'>View</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            <?php
                                            $sql = "SELECT * FROM users WHERE username != '" . ADMIN_NAME . "' ORDER BY regdate DESC LIMIT 5";
                                            $result = $db->prepare($sql);
                                            $result->execute();
                                            while ($row = $result->fetch()) {
                                                $regdate = $row['regdate'];
                                                $lastlogin = $adminfunctions->displayDate($row['timestamp']);
                                                $reg = $adminfunctions->displayDate($row['regdate']);
                                                $email = $row['email'];
                                                $email = strlen($email) > 25 ? substr($email, 0, 25) . "..." : $email;

                                                echo "<tr>";
                                                echo "<td><a href='" . $configs->getConfig('WEB_ROOT') . "adminuseredit.php?usertoedit=" . $row['username'] . "'>" . wordwrap($row['username'],15,"<br>\n",TRUE) . "</a></td><td>" . $adminfunctions->displayStatus($row['username']) . "</td>";
                                                echo "<td>" . $reg . "</td><td>" . $lastlogin . "</td>";
                                                echo "<td class='text-center'><div class='btn-group btn-group-xs'><a href='" . $configs->getConfig('WEB_ROOT') . "adminuseredit.php?usertoedit=" . $row['username'] . "' title='Edit' class='open_modal btn btn-default'><i class='fa fa-pencil'></i> View</a></div></td>";
                                                echo "</tr>";
                                            }
                                            ?>
                                        </tbody>
                                    </table>
                                </div>
                            </div>
                        </div><!-- END Column -->
                    
                        <div class="col-md-12 col-lg-6">
                            <div class="panel">
                                <div class="panel-heading">
                                    <div class="panel-options pull-right">
                                        <button class="btn btn-sm expand-panel"><i class="oi oi-fullscreen-enter"></i></button>
                                        <button class="btn btn-sm close-panel"><i class="oi oi-x"></i></button>
                                        <button class="btn btn-sm minimise-panel"><i class="oi oi-minus"></i></button>
                                    </div>
                                    <i class="oi oi-people"></i><h2 class="panel-title">Users Online</h2>
                                </div>
                                <div class="panel-body">
                                    <?php
                                    $stmt = $session->db->query("SELECT username FROM active_users ORDER BY timestamp DESC,username");
                                    $num_rows = $stmt->columnCount();
                                    if ($num_rows > 1) {
                                        $divider = ',';
                                    } else {
                                        $divider = '';
                                    }
                                    while ($row = $stmt->fetch(PDO::FETCH_ASSOC)) {
                                        echo " <a href='adminuseredit.php?usertoedit=" . $row['username'] . "'>" . $row['username'] . "</a> " . $divider;
                                    }
                                    echo " and " . $session->calcNumActiveGuests() . " guests viewing the site.";
                                    ?>
                                </div>
                            </div>
                            <div class="panel">
                                <div class="panel-heading">
                                    <div class="panel-options pull-right">
                                        <button class="btn btn-sm expand-panel"><i class="oi oi-fullscreen-enter"></i></button>
                                        <button class="btn btn-sm close-panel"><i class="oi oi-x"></i></button>
                                        <button class="btn btn-sm minimise-panel"><i class="oi oi-minus"></i></button>
                                    </div>
                                    <i class="oi oi-people"></i><h2 class="panel-title">User Stats</h2>
                                </div>
                                <div class="panel-body table-responsive">
                                    <div id="basic-column-chart2"></div>
                                </div>
                            </div>                           
                        </div>
                    
                </div>
                <!-- END Row -->

                <footer>Copyright &copy; <?php echo date("Y"); ?> <a href="http://ipm.ir" target="_blank">IPM</a> - All rights reserved. </footer>

            </div>
            <!-- END Page Content -->
            
            <?php include('rightsidebar.php'); ?>

        </div>
        <!-- END Page Wrapper -->
            
        <!-- Scroll to top -->
        <a href="#" id="to-top" class="to-top"><i class="oi oi-chevron-top"></i></a>

        <!-- JavaScript Resources -->
        <script src="js/jquery-2.1.3.min.js"></script>
        <script src="js/bootstrap.min.js"></script>
        <script src="js/plugins/metisMenu/jquery.metisMenu.js"></script>
        <script src="js/komeil.js"></script>     
        
        <!-- Highcharts JS Files -->
        <script src="js/plugins/charts/highCharts/highcharts.js"></script>     
            
        <?php 
        $sql = "SELECT FROM_UNIXTIME(`regdate`, '%M, %Y') AS `date`,
        COUNT(`users`.`id`) AS `count`
        FROM `users`
        GROUP BY `date` ORDER BY `regdate`";
    
        $result = $db->prepare($sql);
        $result->execute();
    
        while ($row = $result->fetch()) { 
            $date[] = $row['date'];
            $count[] = $row['count'];    
        } 
        ?>
        
        <script>
        $(function () {
            $('#basic-column-chart2').highcharts({
        chart: {
            type: 'column'
        },
        title: {
            text: 'User Registrations by Month'
        },
        subtitle: {
            text: 'Source: komeil dashboard'
        },
        xAxis: {
            categories: ['<?php echo join($date, "', '") ?>'],
            crosshair: true
        },
        yAxis: {
            min: 0,
            title: {
                text: 'New Users (number of)'
            }
        },
        tooltip: {
            headerFormat: '<span style="font-size:10px">{point.key}</span><table>',
            pointFormat: '<tr><td style="color:{series.color};padding:0">{series.name}: </td>' +
                    '<td style="padding:0"><b>{point.y:f}</b></td></tr>',
            footerFormat: '</table>',
            shared: true,
            useHTML: true
        },
        plotOptions: {
            column: {
                pointPadding: 0,
                borderWidth: 0
            }
        },
        series: [{
                name: 'Registrations',
                data: [<?php echo join($count, ', ') ?>]
            }]
            });
        });
        </script>

    </body>
</html>
<?php
}
?>